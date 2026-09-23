//! The substrate binding — a **typed absence**.
//!
//! The journal, its encoding, the choice log, the scripted source and the lift are
//! substrate-independent and live in this crate today. What does not exist is the
//! binding that makes the substrate's own lifecycle primitives the source of events.
//! The `asupersync` dependency is not declared in this workspace. Declaring it waits on
//! a licensee decision recorded on bone bn-lf4i (2026-09-22), and until it is declared
//! nothing in this crate names a substrate type or API.
//!
//! The absence is a value, not a comment, so a caller that needs real-code events gets a
//! typed answer — [`InconclusiveReason::Unsupported`] — and never an empty journal that
//! looks like a run in which nothing happened (INV-008).
//!
//! When the binding lands it is one more event source beside
//! [`crate::source::record`]: it emits the same [`crate::family::EventBody`] values into
//! the same [`crate::journal::Journal`], and [`crate::lift::lift`] holds it to the same
//! region calculus. Nothing downstream of the journal changes.

use core::fmt;

use continuum_value::assurance::InconclusiveReason;

/// Why the substrate binding is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAbsence {
    /// The substrate dependency is not declared in this workspace.
    DependencyNotDeclared,
}

impl BindingAbsence {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::DependencyNotDeclared => "dependency-not-declared",
        }
    }
}

impl fmt::Display for BindingAbsence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Whether events can come from the real substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstrateBinding {
    /// They cannot, for this reason.
    Absent(BindingAbsence),
}

impl SubstrateBinding {
    /// The INV-008 reason a request for substrate events is inconclusive.
    #[must_use]
    pub const fn inconclusive_reason(self) -> InconclusiveReason {
        match self {
            Self::Absent(_) => InconclusiveReason::Unsupported,
        }
    }
}

/// The binding this build has.
#[must_use]
pub const fn substrate_binding() -> SubstrateBinding {
    SubstrateBinding::Absent(BindingAbsence::DependencyNotDeclared)
}
