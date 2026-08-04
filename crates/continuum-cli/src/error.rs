//! The CLI's own error taxonomy: the two ways a command can stop before it has an answer to
//! render.
//!
//! [`crate::contract`] owns the exit-code taxonomy as a whole; this module is the half of it
//! that no wire answer reaches. A command line that does not parse is
//! [`crate::contract::Exit::Declined`] (`1`) — the daemon declined nothing because nothing
//! was asked, and the conventions doc's "user error" is the same bucket — and a connection
//! that failed below the protocol is [`crate::contract::Exit::Fault`] (`2`).
//!
//! A daemon's typed refusal is deliberately **not** a [`CliError`]: it is a rendered answer
//! with its own exit code, the same distinction `continuum_mcp::Outcome` draws between a
//! [`ClientError`](continuumd::codec::CodecError) and a
//! [`Refused`](continuumd::protocol::vocabulary::ErrorCode) outcome — a refusal is an
//! *answer*, not a failure to get one. It lands on `1` as well, because a refusal is also
//! "no answer to the question"; what distinguishes the two is the typed
//! [`crate::render::Depth`] and [`crate::wire::Refusal`] in the output, which a fault has no
//! way to carry.

use core::fmt;

use crate::contract::Exit;
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

    /// Where this error sits in [`crate::contract`]'s taxonomy.
    #[must_use]
    pub const fn exit(&self) -> Exit {
        match self {
            Self::Usage(_) => Exit::Declined,
            Self::Connection(_) => Exit::Fault,
        }
    }

    /// The exit code this error's class earns: `1` for a command line that does not parse,
    /// `2` for a connection that failed below the protocol ([`CliError::exit`]).
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        self.exit().code()
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
        // …and both name the taxonomy class the code comes from, so a reader of one is a
        // reader of the other (bn-ybh1z).
        assert_eq!(CliError::usage("bad flag").exit(), Exit::Declined);
        assert_eq!(
            CliError::Connection("no answer".to_owned()).exit(),
            Exit::Fault
        );
    }
}
