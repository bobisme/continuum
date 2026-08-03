//! Output format resolution (`.agents/edict/design/cli-conventions.md`, "Output Formats").
//!
//! Three formats, one audience each: [`Format::Text`] for agents and pipes, [`Format::Pretty`]
//! for a human at a terminal, [`Format::Json`] for a machine that parses. The convention fixes
//! the resolution order — an explicit flag, then the `FORMAT` environment variable, then
//! TTY auto-detection — and this module is that order as a pure function ([`resolve`]) rather
//! than three `if`s scattered across the binary.

use core::fmt;

/// One of the three output formats the conventions doc names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Concise, token-efficient plain text. The default for a non-TTY (pipes, agents).
    Text,
    /// Tables and color for a human at an interactive terminal. Never machine-parsed.
    Pretty,
    /// A structured, stable-schema object envelope. Explicit only.
    Json,
}

impl Format {
    /// Parse a `--format` value, or the `FORMAT` environment variable's value.
    ///
    /// # Errors
    ///
    /// [`UnknownFormat`] carrying the rejected token, for a caller to report as a typed
    /// usage error rather than a bare parse failure.
    pub fn parse(token: &str) -> Result<Self, UnknownFormat> {
        match token {
            "text" => Ok(Self::Text),
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            _ => Err(UnknownFormat(token.to_owned())),
        }
    }
}

/// A `--format`/`FORMAT` token this binary does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFormat(String);

impl UnknownFormat {
    /// The rejected token, verbatim.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnknownFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown format {:?}; expected one of text, pretty, json",
            self.0
        )
    }
}

impl core::error::Error for UnknownFormat {}

/// Resolve the format a command should render in.
///
/// Resolution order, exactly as the conventions doc states it: an explicit flag (`--format`
/// or its hidden `--json` alias) wins outright; failing that, the `FORMAT` environment
/// variable; failing that, TTY auto-detection — [`Format::Pretty`] at an interactive
/// terminal, [`Format::Text`] otherwise (a pipe, a redirect, an agent).
#[must_use]
pub fn resolve(explicit: Option<Format>, env: Option<&str>, stdout_is_tty: bool) -> Format {
    if let Some(format) = explicit {
        return format;
    }
    if let Some(token) = env {
        if let Ok(format) = Format::parse(token) {
            return format;
        }
    }
    if stdout_is_tty {
        Format::Pretty
    } else {
        Format::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_flag_wins_over_everything() {
        assert_eq!(
            resolve(Some(Format::Json), Some("text"), true),
            Format::Json
        );
    }

    #[test]
    fn the_env_var_wins_over_tty_autodetect() {
        assert_eq!(resolve(None, Some("json"), true), Format::Json);
    }

    #[test]
    fn a_malformed_env_var_falls_through_to_autodetect() {
        assert_eq!(resolve(None, Some("xml"), true), Format::Pretty);
        assert_eq!(resolve(None, Some("xml"), false), Format::Text);
    }

    #[test]
    fn autodetect_is_pretty_at_a_tty_and_text_off_one() {
        assert_eq!(resolve(None, None, true), Format::Pretty);
        assert_eq!(resolve(None, None, false), Format::Text);
    }

    #[test]
    fn parse_accepts_exactly_the_three_documented_tokens() {
        assert_eq!(Format::parse("text"), Ok(Format::Text));
        assert_eq!(Format::parse("pretty"), Ok(Format::Pretty));
        assert_eq!(Format::parse("json"), Ok(Format::Json));
        assert!(Format::parse("yaml").is_err());
    }
}
