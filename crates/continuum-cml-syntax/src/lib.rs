//! `continuum-cml-syntax` — CML surface syntax and parser (PR 15a, docs/11).
//!
//! # Responsibility
//!
//! Lexer, grammar, and concrete syntax tree for the CML Finite core fragment — enough
//! surface for the replicated register and the Wave 0 corpus ports.
//!
//! CML is a second front end, not a replacement: the programmatic model API stays
//! supported and both must elaborate to the same semantic model identity.
//!
//! # What this crate delivers
//!
//! - [`parse`] turns `.ctm` source into a [`SourceFile`] tree, or into exactly one typed,
//!   source-located [`ParseError`] — the first defect in source order. The grammar is in
//!   the [`parser`] module documentation.
//! - The tree is deterministic: it is a pure function of the source text, holds no
//!   hash-ordered collections, and every node carries a [`Span`].
//! - A construct that the CML design names (RFC 0003, docs/11, the Revision-2 corpus
//!   fixtures) but that lies outside the Finite core fragment — procedural processes,
//!   parameterized models, views, Forge holes, liveness declarations, symmetry, `choose`,
//!   the temporal `next`, floats — is a [`ParseErrorKind::Unsupported`] error naming the
//!   construct. It is never accepted and never approximated.
//! - [`dump`] renders a tree as a canonical S-expression with spans left out (the golden
//!   form), and [`print`] renders it back to CML source that re-parses to the same tree.
//!
//! The parser assigns no meaning. Name resolution, typing, and lowering to relations
//! belong to `continuum-cml-elab`.
//!
//! # Dependency-boundary contract
//!
//! - INV-016 — source is untrusted data. Parsing may not carry authority, mutate
//!   semantic state, or trigger effects. The parser holds no state beyond one call,
//!   bounds nesting at [`MAX_NESTING`] and input size at [`MAX_SOURCE_BYTES`], and has
//!   no panicking path on any input.
//! - Syntax only: elaboration lives in `continuum-cml-elab`, so this crate does not
//!   import the model core. It has no dependencies at all.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod ast;
pub mod dump;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod print;
pub mod span;

pub use ast::SourceFile;
pub use error::{ParseError, ParseErrorKind, Unsupported};
pub use parser::{parse, parse_expr, parse_type};
pub use span::Span;

/// The deepest nesting of expressions, types, and blocks the parser accepts.
///
/// The parser is recursive descent, so this bound is what keeps hostile input from
/// exhausting the stack (INV-016). Each operator of an infix chain counts as one level,
/// because it deepens the tree by one: the bound therefore also bounds the depth of every
/// tree the parser returns, and so the recursion of every later pass over it. Real models
/// nest a few levels deep; an invariant may conjoin up to this many clauses in one
/// expression, and any number as separate lines of its block.
///
/// The value is measured, not guessed: at this depth every nesting form parses, dumps,
/// prints, and drops within half of a default 2 MiB thread stack in an unoptimized build
/// (`tests/errors.rs`, `adversarial_maximum_depth_fits_in_half_a_default_stack`).
pub const MAX_NESTING: u32 = 64;

/// The largest source the parser accepts, in bytes (16 MiB).
///
/// Spans store byte offsets as `u32`; this bound keeps every offset representable.
pub const MAX_SOURCE_BYTES: u32 = 16 * 1024 * 1024;
