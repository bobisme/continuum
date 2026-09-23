//! Controlled choice logs: the scheduler's decisions, written down as data.
//!
//! docs/01 §6 maps the substrate's "Lab scheduler choice" to Continuum's "replay
//! choice". This type is the Continuum half of that row, defined here and owned here:
//! no substrate type appears in it, so a log is meaningful with the substrate absent and
//! stays meaningful now that the binding has landed ([`crate::binding::run`] takes the same
//! log).
//!
//! # What one choice means
//!
//! At each step some set of actors can move. The *enabled* actors are those with work
//! left, in ascending actor index. A [`Choice`] is an index into that enabled list — not
//! an actor id — so every log over a given set of scripts is a valid schedule exactly
//! when each choice is in range, and the log alone, with the scripts, fixes the run
//! (INV-005: scheduling reaches controlled code only as this explicit value).

use core::fmt;

/// One scheduler decision: the index of the chosen actor among those enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Choice(pub u32);

/// A sequence of scheduler decisions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChoiceLog(Vec<Choice>);

impl ChoiceLog {
    /// A log of these choices, in this order.
    #[must_use]
    pub fn new(choices: impl IntoIterator<Item = u32>) -> Self {
        Self(choices.into_iter().map(Choice).collect())
    }

    /// The choices.
    #[must_use]
    pub fn choices(&self) -> &[Choice] {
        &self.0
    }

    /// How many choices.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every complete log for actors with these many steps each, in a deterministic
    /// order (lexicographic by choice).
    ///
    /// For lengths `n₁ … nₖ` there are `(Σnᵢ)! / Πnᵢ!` logs, so callers keep inputs small
    /// and the enumeration exhaustive, as `continuum_task::region::schedule::interleavings`
    /// does for schedules.
    #[must_use]
    pub fn enumerate(lengths: &[usize]) -> Vec<Self> {
        let mut out = Vec::new();
        let mut remaining = lengths.to_vec();
        let mut prefix = Vec::new();
        weave(&mut remaining, &mut prefix, &mut out);
        out
    }
}

fn weave(remaining: &mut [usize], prefix: &mut Vec<Choice>, out: &mut Vec<ChoiceLog>) {
    let enabled: Vec<usize> = (0..remaining.len()).filter(|i| remaining[*i] > 0).collect();
    if enabled.is_empty() {
        out.push(ChoiceLog(prefix.clone()));
        return;
    }
    for (index, actor) in enabled.iter().enumerate() {
        remaining[*actor] -= 1;
        prefix.push(Choice(u32::try_from(index).unwrap_or(u32::MAX)));
        weave(remaining, prefix, out);
        prefix.pop();
        remaining[*actor] += 1;
    }
}

impl fmt::Display for ChoiceLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|c| c.0.to_string()).collect();
        write!(f, "[{}]", parts.join(","))
    }
}
