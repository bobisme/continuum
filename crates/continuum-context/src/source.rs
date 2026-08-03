//! Source spans: the `selected[].kind = "source"` half of PR-11 / IMPL-03 (RFC 0028,
//! "Required fields, reconciled with plan §6.2", the "relevant source spans and model
//! actions" row).
//!
//! # Shape
//!
//! The IDL already fixes a `SourceSpan` shape, for `Diagnostic.span` and for
//! `correspondence.bind`'s `source: SourceSpan required` (plan §16's correspondence
//! graph):
//!
//! ```text
//! struct SourceSpan {
//!   file: String required;
//!   start_line: U32 required;
//!   start_column: U32 required;
//!   end_line: U32 required;
//!   end_column: U32 required;
//! }
//! ```
//!
//! — `notes/plan/schemas/continuumd-native-protocol.idl`
//!
//! [`SourceSpan`] mirrors it field for field, with one typed strengthening: `file` is a
//! [`WorkspacePath`](continuum_workspace::snapshot::WorkspacePath), the type
//! `continuum-workspace` already established for "a path to one file inside a workspace,
//! relative to the workspace root" — segment-wise validated, no `.`/`..`, no traversal —
//! rather than the IDL's bare `String`. That is a strengthening the IDL leaves room for
//! (nothing about the wire shape requires an *unvalidated* string) and not a second
//! spelling of it: [`SourceSpan`]'s `Display` renders `file` through
//! `WorkspacePath::to_string`, which is the same `/`-joined form the wire field would
//! carry.
//!
//! Line and column are 1-based, matching the near-universal compiler/editor convention
//! (rustc among them) — a choice the IDL does not fix, so it is stated here rather than
//! left to be inferred from a test.
//!
//! # INV-016: a position, never a snippet
//!
//! > Source is untrusted data and MUST NOT be interpolated into any description
//! > (INV-016).
//! >
//! > — RFC 0028
//!
//! [`SourceSpan`] has no field for source text and this module reads no file: it is a
//! position, constructed from typed integers and a typed path, and
//! [`SourceRef::into_selected_item`] renders a summary from that position alone (see
//! `crate::selection`'s "Why a `SelectedItem` cannot carry caller-supplied prose"). There
//! is structurally no argument through which file content could reach a pack.
//!
//! # What is declined
//!
//! - **Reading a file, or converting a byte offset to a line/column.** This crate has no
//!   I/O boundary (mirroring `continuum-workspace`'s own "no Continuum dependency […]
//!   still" leaf discipline one layer up); a caller that has a byte offset converts it
//!   before calling [`SourceSpan::new`].
//! - **A standalone artifact for a source span.** Plan §4.4's class list has no `src_*`
//!   entry — a span is a position inside the pack's `ws_*` snapshot, not a separate
//!   content-addressed artifact — so [`SourceRef::into_selected_item`] always emits
//!   `artifact: null`, and there is no field here to populate one by mistake.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | mirrors the IDL's five `SourceSpan` fields | the IDL, above | `a_span_renders_file_line_column` |
//! | 1-based lines and columns | this module's own stated convention | `line_or_column_zero_is_refused` |
//! | end is not before start | this module's own stated convention | `an_end_before_start_is_refused`, anti-vacuity `a_well_ordered_span_is_accepted` |
//! | a point span (start == end) is valid | this module's own stated convention | `a_point_span_is_accepted` |
//! | kind is `source` | `context-pack.schema.json` | `a_source_ref_projects_to_the_source_kind` |
//! | `artifact` is always `null` | plan §4.4 (no `src_*` class) | `a_source_ref_never_carries_an_artifact` |
//! | the summary is a pure function of the span, never of caller text | INV-016 | `summary_is_exactly_the_spans_display`, and the method signature itself (no string parameter) |
//! | two distinct spans project to two distinct items | `rule ordering.deterministic` | `distinct_spans_project_to_distinct_items` |

use core::fmt;

use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;

use crate::selection::{SelectedItem, SelectionKind};

/// A source location: a file, inclusive-start/exclusive-in-spirit end line and column
/// (see the module documentation for the exact convention).
///
/// `Ord` is derivation-order over `(file, start_line, start_column, end_line,
/// end_column)`, which is a total order and agrees with structural equality — two spans
/// compare equal exactly when every field does.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceSpan {
    file: WorkspacePath,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

impl SourceSpan {
    /// Build a span.
    ///
    /// # Errors
    ///
    /// [`SourceSpanError::ZeroLine`] or [`SourceSpanError::ZeroColumn`] when a line or
    /// column is `0` (this module's lines and columns are 1-based), and
    /// [`SourceSpanError::EndBeforeStart`] when the end position precedes the start
    /// position under `(line, column)` lexicographic order.
    pub fn new(
        file: WorkspacePath,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Result<Self, SourceSpanError> {
        if start_line == 0 {
            return Err(SourceSpanError::ZeroLine {
                at: Position::Start,
            });
        }
        if end_line == 0 {
            return Err(SourceSpanError::ZeroLine { at: Position::End });
        }
        if start_column == 0 {
            return Err(SourceSpanError::ZeroColumn {
                at: Position::Start,
            });
        }
        if end_column == 0 {
            return Err(SourceSpanError::ZeroColumn { at: Position::End });
        }
        if (end_line, end_column) < (start_line, start_column) {
            return Err(SourceSpanError::EndBeforeStart);
        }
        Ok(Self {
            file,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    /// The file this span is inside, relative to the pack's `ws_*` snapshot.
    #[must_use]
    pub const fn file(&self) -> &WorkspacePath {
        &self.file
    }

    /// The 1-based start line.
    #[must_use]
    pub const fn start_line(&self) -> u32 {
        self.start_line
    }

    /// The 1-based start column.
    #[must_use]
    pub const fn start_column(&self) -> u32 {
        self.start_column
    }

    /// The 1-based end line.
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        self.end_line
    }

    /// The 1-based end column.
    #[must_use]
    pub const fn end_column(&self) -> u32 {
        self.end_column
    }
}

impl fmt::Display for SourceSpan {
    /// `file:start_line:start_column-end_line:end_column` — a position, never a snippet.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}-{}:{}",
            self.file, self.start_line, self.start_column, self.end_line, self.end_column
        )
    }
}

/// Which end of a span a [`SourceSpanError`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// The span's start.
    Start,
    /// The span's end.
    End,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Start => "start",
            Self::End => "end",
        })
    }
}

/// Why a [`SourceSpan`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSpanError {
    /// A line was `0`; this module's lines are 1-based.
    ZeroLine {
        /// Which position named the offending line.
        at: Position,
    },
    /// A column was `0`; this module's columns are 1-based.
    ZeroColumn {
        /// Which position named the offending column.
        at: Position,
    },
    /// The end position precedes the start position.
    EndBeforeStart,
}

impl fmt::Display for SourceSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLine { at } => write!(f, "{at} line is 0; lines are 1-based"),
            Self::ZeroColumn { at } => write!(f, "{at} column is 0; columns are 1-based"),
            Self::EndBeforeStart => f.write_str("end position precedes start position"),
        }
    }
}

impl core::error::Error for SourceSpanError {}

/// A typed reference to a source span: the `selected[].kind = "source"` selection kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRef {
    span: SourceSpan,
}

impl SourceRef {
    /// Reference `span`.
    #[must_use]
    pub const fn new(span: SourceSpan) -> Self {
        Self { span }
    }

    /// The span this reference names.
    #[must_use]
    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// Project this reference into the pack's generic `selected[]` shape, under `id`.
    ///
    /// `artifact` is always `null` (see the module documentation) and `summary` is
    /// exactly `self.span.to_string()` — a pure function of the typed span, with no
    /// string parameter through which caller-supplied text could enter (INV-016).
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        let summary = self.span.to_string();
        SelectedItem::new(id, SelectionKind::Source, summary, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("well-formed test path")
    }

    fn id(text: &str) -> Name {
        Name::new(text).expect("well-formed test id")
    }

    #[test]
    fn a_span_renders_file_line_column() {
        let span = SourceSpan::new(path("crates/foo/src/lib.rs"), 12, 5, 14, 2)
            .expect("well-ordered span");
        assert_eq!(span.to_string(), "crates/foo/src/lib.rs:12:5-14:2");
        assert_eq!(span.file(), &path("crates/foo/src/lib.rs"));
        assert_eq!((span.start_line(), span.start_column()), (12, 5));
        assert_eq!((span.end_line(), span.end_column()), (14, 2));
    }

    #[test]
    fn a_point_span_is_accepted() {
        let span = SourceSpan::new(path("a.rs"), 3, 7, 3, 7).expect("a point span is a span");
        assert_eq!(span.to_string(), "a.rs:3:7-3:7");
    }

    #[test]
    fn a_well_ordered_multiline_span_is_accepted() {
        // Anti-vacuity companion to `an_end_before_start_is_refused`: the ordering check
        // must actually admit an ordered span, not reject everything.
        assert!(SourceSpan::new(path("a.rs"), 1, 1, 2, 1).is_ok());
        assert!(SourceSpan::new(path("a.rs"), 5, 9, 5, 20).is_ok());
    }

    #[test]
    fn an_end_before_start_is_refused() {
        assert_eq!(
            SourceSpan::new(path("a.rs"), 5, 1, 4, 1),
            Err(SourceSpanError::EndBeforeStart)
        );
        assert_eq!(
            SourceSpan::new(path("a.rs"), 5, 10, 5, 9),
            Err(SourceSpanError::EndBeforeStart)
        );
    }

    #[test]
    fn line_or_column_zero_is_refused() {
        assert_eq!(
            SourceSpan::new(path("a.rs"), 0, 1, 1, 1),
            Err(SourceSpanError::ZeroLine {
                at: Position::Start
            })
        );
        assert_eq!(
            SourceSpan::new(path("a.rs"), 1, 1, 0, 1),
            Err(SourceSpanError::ZeroLine { at: Position::End })
        );
        assert_eq!(
            SourceSpan::new(path("a.rs"), 1, 0, 1, 1),
            Err(SourceSpanError::ZeroColumn {
                at: Position::Start
            })
        );
        assert_eq!(
            SourceSpan::new(path("a.rs"), 1, 1, 1, 0),
            Err(SourceSpanError::ZeroColumn { at: Position::End })
        );
    }

    #[test]
    fn a_source_ref_projects_to_the_source_kind() {
        let span = SourceSpan::new(path("a.rs"), 1, 1, 1, 5).expect("well-ordered");
        let item = SourceRef::new(span).into_selected_item(id("e_span"));
        assert_eq!(item.kind(), SelectionKind::Source);
        assert_eq!(item.id().as_str(), "e_span");
    }

    #[test]
    fn a_source_ref_never_carries_an_artifact() {
        let span = SourceSpan::new(path("a.rs"), 1, 1, 1, 5).expect("well-ordered");
        let item = SourceRef::new(span).into_selected_item(id("e_span"));
        assert_eq!(item.artifact(), None);
        assert!(item.to_json().as_object().unwrap()["artifact"].is_null());
    }

    #[test]
    fn summary_is_exactly_the_spans_display() {
        let span = SourceSpan::new(path("a.rs"), 2, 3, 2, 9).expect("well-ordered");
        let rendered = span.to_string();
        let item = SourceRef::new(span).into_selected_item(id("e_span"));
        assert_eq!(item.summary(), rendered);
    }

    #[test]
    fn distinct_spans_project_to_distinct_items() {
        let a = SourceRef::new(SourceSpan::new(path("a.rs"), 1, 1, 1, 2).expect("ok"))
            .into_selected_item(id("x"));
        let b = SourceRef::new(SourceSpan::new(path("a.rs"), 1, 1, 1, 3).expect("ok"))
            .into_selected_item(id("x"));
        assert_ne!(a, b);
        assert_ne!(a.to_canonical_bytes(), b.to_canonical_bytes());
    }
}
