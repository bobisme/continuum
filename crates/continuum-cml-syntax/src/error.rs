//! Typed, source-located parse errors.
//!
//! A parse either produces a tree or exactly one [`ParseError`]: the first defect in
//! source order. Three families are kept distinct because they mean different things to
//! a caller (INV-008):
//!
//! - **malformed** — the text is not CML at all ([`ParseErrorKind::UnexpectedCharacter`],
//!   [`ParseErrorKind::UnexpectedToken`], …);
//! - **unsupported** — the text names a construct that exists in the CML design
//!   (RFC 0003, docs/11) but lies outside the Finite core fragment this parser accepts
//!   (PR 15a). It is reported as [`ParseErrorKind::Unsupported`], never accepted and
//!   never approximated;
//! - **resource** — the input exceeds a hard parser bound
//!   ([`ParseErrorKind::NestingTooDeep`], [`ParseErrorKind::InputTooLarge`]). Source is
//!   untrusted data (INV-016), so the parser refuses rather than overflows.

use std::fmt;

use crate::span::Span;

/// The single error a failed parse returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// What went wrong.
    pub kind: ParseErrorKind,
    /// Where it went wrong.
    pub span: Span,
}

impl ParseError {
    /// A stable, machine-readable code for the error family and kind.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// Whether this error reports an out-of-fragment construct rather than malformed
    /// or oversized input.
    #[must_use]
    pub fn is_unsupported(&self) -> bool {
        matches!(self.kind, ParseErrorKind::Unsupported(_))
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.span, self.code(), self.kind)
    }
}

impl std::error::Error for ParseError {}

/// The kinds of parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// A character that begins no CML token.
    UnexpectedCharacter(char),
    /// A string literal with no closing quote before the end of its line.
    UnterminatedString,
    /// A string escape other than `\"`, `\\`, or `\n`.
    InvalidEscape(char),
    /// An integer literal that does not fit in 64 bits.
    IntegerTooLarge,
    /// A token that the grammar does not allow here.
    UnexpectedToken {
        /// What the grammar allows here.
        expected: &'static str,
        /// The token found, as written in the source (or `end of input`).
        found: String,
    },
    /// A non-associative operator used twice at one level, such as `a == b == c`;
    /// the source must parenthesize.
    ChainedOperator(&'static str),
    /// A statement that the enclosing block does not allow, such as `next` in an
    /// invariant.
    StatementNotAllowed {
        /// The statement keyword.
        statement: &'static str,
        /// The kind of block it appeared in.
        context: &'static str,
    },
    /// A construct outside the Finite core fragment.
    Unsupported(Unsupported),
    /// Expression, type, or block nesting beyond [`crate::MAX_NESTING`].
    NestingTooDeep,
    /// Source longer than [`crate::MAX_SOURCE_BYTES`].
    InputTooLarge,
}

impl ParseErrorKind {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            ParseErrorKind::UnexpectedCharacter(_) => "cml.lex.unexpected_character",
            ParseErrorKind::UnterminatedString => "cml.lex.unterminated_string",
            ParseErrorKind::InvalidEscape(_) => "cml.lex.invalid_escape",
            ParseErrorKind::IntegerTooLarge => "cml.lex.integer_too_large",
            ParseErrorKind::UnexpectedToken { .. } => "cml.parse.unexpected_token",
            ParseErrorKind::ChainedOperator(_) => "cml.parse.chained_operator",
            ParseErrorKind::StatementNotAllowed { .. } => "cml.parse.statement_not_allowed",
            ParseErrorKind::Unsupported(u) => u.code(),
            ParseErrorKind::NestingTooDeep => "cml.limit.nesting_too_deep",
            ParseErrorKind::InputTooLarge => "cml.limit.input_too_large",
        }
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::UnexpectedCharacter(c) => {
                write!(f, "unexpected character {c:?}")
            }
            ParseErrorKind::UnterminatedString => write!(f, "unterminated string literal"),
            ParseErrorKind::InvalidEscape(c) => write!(f, "invalid string escape \\{c}"),
            ParseErrorKind::IntegerTooLarge => {
                write!(f, "integer literal does not fit in 64 bits")
            }
            ParseErrorKind::UnexpectedToken { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            ParseErrorKind::ChainedOperator(op) => {
                write!(f, "operator `{op}` does not chain; add parentheses")
            }
            ParseErrorKind::StatementNotAllowed { statement, context } => {
                write!(f, "`{statement}` is not allowed in {context}")
            }
            ParseErrorKind::Unsupported(u) => write!(
                f,
                "{} is outside the CML Finite core fragment: {}",
                u.construct(),
                u.reason()
            ),
            ParseErrorKind::NestingTooDeep => write!(
                f,
                "nesting deeper than {} levels is refused",
                crate::MAX_NESTING
            ),
            ParseErrorKind::InputTooLarge => write!(
                f,
                "source longer than {} bytes is refused",
                crate::MAX_SOURCE_BYTES
            ),
        }
    }
}

/// A construct that the CML design names but the Finite core fragment does not accept.
///
/// Each variant names the construct, not a guess at its meaning; [`Unsupported::reason`]
/// says which fragment or later PR owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unsupported {
    /// `model Name<…>` — a model parameterized over constants or sorts.
    ParameterizedModel,
    /// `Name<…>` in a type — a sort indexed by a parameter.
    ParameterizedSort,
    /// `process` — procedural processes with program counters.
    Process,
    /// `await` — a blocking guard in procedural code.
    Await,
    /// `goto` — a program-counter transfer in procedural code.
    Goto,
    /// `view` / `refines` — refinement views.
    View,
    /// `forge` — a Forge synthesis block.
    ForgeBlock,
    /// `??name` — a synthesis hole.
    SynthesisHole,
    /// `progress` — a liveness property declaration.
    ProgressProperty,
    /// `eventually Name { … }` — a liveness property declaration.
    EventuallyProperty,
    /// `transition invariant` — a trace property over events.
    TransitionInvariant,
    /// `hyperproperty` — a property over several traces.
    Hyperproperty,
    /// `symmetry` — a symmetry declaration.
    Symmetry,
    /// `check` — a checking directive inside the source.
    CheckDirective,
    /// `import` / `use` — module composition.
    ModuleImport,
    /// `extern` / `opaque` — opaque external domains.
    OpaqueDomain,
    /// `choose` — explicit nondeterministic choice.
    Choose,
    /// `next(…)` used as a temporal operator in an expression.
    TemporalNext,
    /// A floating-point literal such as `1.5`.
    FloatLiteral,
}

impl Unsupported {
    /// Every unsupported construct, in declaration order.
    pub const ALL: [Unsupported; 19] = [
        Unsupported::ParameterizedModel,
        Unsupported::ParameterizedSort,
        Unsupported::Process,
        Unsupported::Await,
        Unsupported::Goto,
        Unsupported::View,
        Unsupported::ForgeBlock,
        Unsupported::SynthesisHole,
        Unsupported::ProgressProperty,
        Unsupported::EventuallyProperty,
        Unsupported::TransitionInvariant,
        Unsupported::Hyperproperty,
        Unsupported::Symmetry,
        Unsupported::CheckDirective,
        Unsupported::ModuleImport,
        Unsupported::OpaqueDomain,
        Unsupported::Choose,
        Unsupported::TemporalNext,
        Unsupported::FloatLiteral,
    ];

    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Unsupported::ParameterizedModel => "cml.unsupported.parameterized_model",
            Unsupported::ParameterizedSort => "cml.unsupported.parameterized_sort",
            Unsupported::Process => "cml.unsupported.process",
            Unsupported::Await => "cml.unsupported.await",
            Unsupported::Goto => "cml.unsupported.goto",
            Unsupported::View => "cml.unsupported.view",
            Unsupported::ForgeBlock => "cml.unsupported.forge_block",
            Unsupported::SynthesisHole => "cml.unsupported.synthesis_hole",
            Unsupported::ProgressProperty => "cml.unsupported.progress_property",
            Unsupported::EventuallyProperty => "cml.unsupported.eventually_property",
            Unsupported::TransitionInvariant => "cml.unsupported.transition_invariant",
            Unsupported::Hyperproperty => "cml.unsupported.hyperproperty",
            Unsupported::Symmetry => "cml.unsupported.symmetry",
            Unsupported::CheckDirective => "cml.unsupported.check_directive",
            Unsupported::ModuleImport => "cml.unsupported.module_import",
            Unsupported::OpaqueDomain => "cml.unsupported.opaque_domain",
            Unsupported::Choose => "cml.unsupported.choose",
            Unsupported::TemporalNext => "cml.unsupported.temporal_next",
            Unsupported::FloatLiteral => "cml.unsupported.float_literal",
        }
    }

    /// The construct, as a reader would name it.
    #[must_use]
    pub fn construct(self) -> &'static str {
        match self {
            Unsupported::ParameterizedModel => "a parameterized model",
            Unsupported::ParameterizedSort => "a parameterized sort",
            Unsupported::Process => "`process`",
            Unsupported::Await => "`await`",
            Unsupported::Goto => "`goto`",
            Unsupported::View => "`view`",
            Unsupported::ForgeBlock => "a `forge` block",
            Unsupported::SynthesisHole => "a synthesis hole `??`",
            Unsupported::ProgressProperty => "`progress`",
            Unsupported::EventuallyProperty => "an `eventually` property declaration",
            Unsupported::TransitionInvariant => "`transition invariant`",
            Unsupported::Hyperproperty => "`hyperproperty`",
            Unsupported::Symmetry => "`symmetry`",
            Unsupported::CheckDirective => "a `check` directive",
            Unsupported::ModuleImport => "a module import",
            Unsupported::OpaqueDomain => "an opaque external domain",
            Unsupported::Choose => "`choose`",
            Unsupported::TemporalNext => "the temporal `next` operator",
            Unsupported::FloatLiteral => "a floating-point literal",
        }
    }

    /// Why the Finite core fragment does not accept it, and what owns it.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Unsupported::ParameterizedModel | Unsupported::ParameterizedSort => {
                "parameterized verification is RFC 0016; the Finite core takes its bounds from the run configuration"
            }
            Unsupported::Process | Unsupported::Await | Unsupported::Goto => {
                "procedural-to-relational lowering is Wave 1 (PORTING_WAVES.md), not the Finite core"
            }
            Unsupported::View => "refinement views are RFC 0009 and later than PR 15a",
            Unsupported::ForgeBlock | Unsupported::SynthesisHole => {
                "synthesis holes belong to Forge (RFC 0033), which the verifier never imports"
            }
            Unsupported::ProgressProperty
            | Unsupported::EventuallyProperty
            | Unsupported::TransitionInvariant
            | Unsupported::Hyperproperty => {
                "liveness, trace, and hyperproperty declarations belong to the Temporal fragment (ADR-0025, RFC 0015)"
            }
            Unsupported::Symmetry => "symmetry declarations are RFC 0016 and later than PR 15a",
            Unsupported::CheckDirective => {
                "a run configuration is separate from source (RFC 0003, Configurations and bounds)"
            }
            Unsupported::ModuleImport => {
                "module composition needs explicit coupling declarations (RFC 0003) and is later than PR 15a"
            }
            Unsupported::OpaqueDomain => {
                "opaque domains are not exactly enumerable, so they are not Finite (ADR-0025)"
            }
            Unsupported::Choose => {
                "explicit choice is not in the PR 15a Finite core surface; write the choice as an action parameter"
            }
            Unsupported::TemporalNext => {
                "the temporal core is LTL without next (RFC 0037, RFC 0015); use `next x = e` statements or primes in actions"
            }
            Unsupported::FloatLiteral => {
                "floating-point values are excluded so canonical encodings stay byte-deterministic (RFC 0037)"
            }
        }
    }
}
