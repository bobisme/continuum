//! The handle patterns `repair-transaction.schema.json` fixes for the base triple and
//! the transaction's own identity (RFC 0032, "The transaction object"; ADR-0037
//! explicit content-addressed handles).
//!
//! Plan §4.4: "Handles carry a kind prefix but are otherwise structureless." Each type
//! here checks one schema pattern and nothing more. The base intent reuses
//! `continuum_intent::contract::IntentId` (`^in_[A-Za-z0-9_-]+$`), so there is one
//! spelling of that check.

use core::fmt;

/// A handle that does not match its schema pattern.
///
/// The value is not echoed back: a handle arrives from a caller, and a refusal names
/// the pattern it failed, not the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedHandle {
    /// The schema pattern the value had to match.
    pub pattern: &'static str,
}

impl fmt::Display for MalformedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "handle does not match {}", self.pattern)
    }
}

impl std::error::Error for MalformedHandle {}

const fn is_handle_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn check(handle: &str, prefix: &str, pattern: &'static str) -> Result<(), MalformedHandle> {
    match handle.strip_prefix(prefix) {
        Some(suffix) if !suffix.is_empty() && suffix.chars().all(is_handle_char) => Ok(()),
        _ => Err(MalformedHandle { pattern }),
    }
}

macro_rules! handle_type {
    ($(#[$doc:meta])* $name:ident, $prefix:literal, $pattern:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The schema pattern this handle matches.
            pub const PATTERN: &'static str = $pattern;

            /// Validate and wrap a handle.
            ///
            /// # Errors
            ///
            /// [`MalformedHandle`] when the value does not match [`Self::PATTERN`].
            pub fn new(handle: &str) -> Result<Self, MalformedHandle> {
                check(handle, $prefix, $pattern)?;
                Ok(Self(handle.to_owned()))
            }

            /// The handle.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

handle_type!(
    /// The crashpack a transaction repairs: `failure`, `^crash_[A-Za-z0-9_-]+$`.
    CrashpackId,
    "crash_",
    "^crash_[A-Za-z0-9_-]+$"
);

handle_type!(
    /// A workspace snapshot: `base_snapshot` and `candidate_snapshot`,
    /// `^ws_[A-Za-z0-9_-]+$`.
    SnapshotId,
    "ws_",
    "^ws_[A-Za-z0-9_-]+$"
);

handle_type!(
    /// A transaction version: `repair_id` and `supersedes`, `^rt_[A-Za-z0-9_-]+$`.
    ///
    /// [`crate::transaction::RepairTransaction`] mints its own; this constructor
    /// exists so a reader can check a handle it was given.
    RepairId,
    "rt_",
    "^rt_[A-Za-z0-9_-]+$"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_accept_the_schema_alphabet_and_nothing_else() {
        assert!(CrashpackId::new("crash_a-B_9").is_ok());
        assert!(SnapshotId::new("ws_x").is_ok());
        assert!(RepairId::new("rt_0").is_ok());
        for bad in [
            "crash_",
            "crash",
            "crash_a b",
            "crash_é",
            "ws_x",
            "Crash_a",
            " crash_a",
        ] {
            assert_eq!(
                CrashpackId::new(bad),
                Err(MalformedHandle {
                    pattern: CrashpackId::PATTERN
                }),
                "{bad:?}"
            );
        }
    }
}
