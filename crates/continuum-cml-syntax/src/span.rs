//! Source locations.
//!
//! Decision: RFC 0003 (the CML surface), PR 15a. docs/11 §2 requires source maps to be
//! preserved through normalization, so every node carries a span.
//!
//! Every token, tree node, and error carries a [`Span`]. Offsets are byte offsets into
//! the parsed source; `line` and `col` locate the span's first character, both 1-based,
//! with `col` counted in Unicode scalar values so a location matches what an editor
//! shows.

use std::fmt;

/// A half-open byte range `start..end` in the source, plus the line and column of
/// `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
    /// 1-based line of `start`.
    pub line: u32,
    /// 1-based column of `start`, in Unicode scalar values.
    pub col: u32,
}

impl Span {
    /// The span that starts where `self` starts and ends where `other` ends.
    #[must_use]
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end.max(self.start),
            line: self.line,
            col: self.col,
        }
    }

    /// The length of the span in bytes.
    #[must_use]
    pub fn len(self) -> u32 {
        self.end - self.start
    }

    /// Whether the span covers no bytes (the end-of-input position, for example).
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
