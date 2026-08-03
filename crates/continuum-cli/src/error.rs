//! The CLI's own error taxonomy and its exit-code mapping
//! (`.agents/edict/design/cli-conventions.md`, "Exit Codes").
//!
//! Three exit codes exist in the convention — `0` success, `1` user error, `2` system error
//! — and every failure this crate can produce is classified into one of the two non-zero
//! ones by [`CliError::exit_code`]. A daemon's typed refusal is deliberately **not** an
//! error here: `context`/`task` render a refusal as a successful command that reported a
//! `no` (exit `0`), the same distinction `continuum_mcp::Outcome` draws between a
//! [`ClientError`](continuumd::codec::CodecError) and a
//! [`Refused`](continuumd::protocol::vocabulary::ErrorCode) outcome — a refusal is an
//! *answer*, not a failure to get one.

use core::fmt;

use crate::format::UnknownFormat;
use crate::wire::ConnectError;

/// Something that stopped a command before it could render an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// The command line itself does not parse: a missing flag, an unknown flag, a
    /// malformed handle, an unrecognized verb.
    Usage(String),
    /// The daemon connection or the wire codec failed below the protocol — never a
    /// refusal, which is a typed answer and not this.
    Connection(String),
}

impl CliError {
    /// A usage error naming what was wrong, in one sentence, with no trailing period
    /// (callers append one line of remediation).
    #[must_use]
    pub fn usage(detail: impl Into<String>) -> Self {
        Self::Usage(detail.into())
    }

    /// The exit code the conventions doc assigns this error's class.
    ///
    /// `1` for a user error (bad arguments), `2` for a system error (the wire failed).
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 1,
            Self::Connection(_) => 2,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(detail) => write!(f, "Error: {detail}"),
            Self::Connection(detail) => write!(f, "Error: {detail}"),
        }
    }
}

impl core::error::Error for CliError {}

impl From<UnknownFormat> for CliError {
    fn from(error: UnknownFormat) -> Self {
        Self::usage(format!(
            "{error}\n  Pass --format text, --format pretty, or --format json"
        ))
    }
}

impl From<ConnectError> for CliError {
    fn from(error: ConnectError) -> Self {
        Self::Connection(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_usage_error_exits_one_and_a_connection_error_exits_two() {
        assert_eq!(CliError::usage("bad flag").exit_code(), 1);
        assert_eq!(CliError::Connection("no answer".to_owned()).exit_code(), 2);
    }
}
