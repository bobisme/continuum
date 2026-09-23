//! Typed, source-located elaboration errors.
//!
//! Decision: RFC 0003 ("Predictable elaboration") and INV-008 (typed inconclusiveness).
//! An elaboration produces a normalized model or exactly one [`ElabError`]: the first
//! defect found, in a fixed pass order over source order. Two families are kept apart,
//! because they mean different things to a caller:
//!
//! - **ill-formed** — the model is not a well-formed CML model: an unknown name, a type
//!   mismatch, a state variable an action leaves unspecified;
//! - **unsupported** — the model is well-formed, but it uses semantics outside the
//!   Finite core fragment this elaborator implements (ADR-0025). It is
//!   [`ElabErrorKind::Unsupported`], never accepted and never approximated.
//!
//! Lowering to the programmatic model has its own error, [`crate::lower::LowerError`],
//! because a model can be well-formed CML and still use values that the programmatic
//! model cannot carry.

use std::fmt;

use continuum_cml_syntax::{ParseError, Span};

/// The single error a failed elaboration returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElabError {
    /// What went wrong.
    pub kind: ElabErrorKind,
    /// Where it went wrong.
    pub span: Span,
}

impl ElabError {
    pub(crate) fn new(kind: ElabErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// Whether the error reports semantics outside the fragment rather than an
    /// ill-formed model.
    #[must_use]
    pub fn is_unsupported(&self) -> bool {
        match &self.kind {
            ElabErrorKind::Unsupported(_) => true,
            ElabErrorKind::Parse(e) => e.is_unsupported(),
            _ => false,
        }
    }
}

impl fmt::Display for ElabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.span, self.code(), self.kind)
    }
}

impl std::error::Error for ElabError {}

/// Semantics outside the Finite core fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unsupported {
    /// A `def` that calls itself, directly or through other defs. Totality and
    /// termination of model functions (docs/11 §8) are not checked yet.
    RecursiveDef,
    /// A primed expression that is not `x' == e` for a state variable `x`: a relational
    /// postcondition (docs/11 §4).
    RelationalPostcondition,
    /// An integer literal beyond `i64`.
    IntegerBeyondI64,
}

impl Unsupported {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Unsupported::RecursiveDef => "cml.elab.unsupported.recursive_def",
            Unsupported::RelationalPostcondition => "cml.elab.unsupported.relational_postcondition",
            Unsupported::IntegerBeyondI64 => "cml.elab.unsupported.integer_beyond_i64",
        }
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unsupported::RecursiveDef => {
                "recursive model functions are outside the Finite core fragment"
            }
            Unsupported::RelationalPostcondition => {
                "only `x' == e` for a state variable `x` is supported as a post-state clause"
            }
            Unsupported::IntegerBeyondI64 => "integer literals beyond i64 are not supported",
        })
    }
}

/// The kinds of elaboration error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabErrorKind {
    /// The source did not parse.
    Parse(ParseError),
    /// A name declared twice at the top level, or a local name that shadows another.
    DuplicateName(String),
    /// A name that is not declared, or not in scope here.
    UnknownName(String),
    /// A type name that is not declared.
    UnknownType(String),
    /// A type constructor applied to the wrong number of arguments.
    TypeArity {
        /// The constructor.
        name: String,
        /// How many arguments it takes.
        expected: usize,
        /// How many it was given.
        found: usize,
    },
    /// A type alias that refers to itself.
    CyclicTypeAlias(String),
    /// Two types that must be equal are not.
    TypeMismatch {
        /// The type required here.
        expected: String,
        /// The type found.
        found: String,
    },
    /// A type the uses do not determine, such as a binder no clause constrains.
    CannotInferType(String),
    /// A name used as a value that denotes something else (an action, a type, …).
    NotAValue(String),
    /// A call of something that is not a function, or of an unknown method.
    UnknownFunction(String),
    /// A call with the wrong number of arguments.
    Arity {
        /// The function.
        name: String,
        /// How many arguments it takes.
        expected: usize,
        /// How many it was given.
        found: usize,
    },
    /// A record field that the record type does not have.
    UnknownField(String),
    /// An action leaves a state variable unspecified (docs/11 §4).
    UnspecifiedStateChange {
        /// The action.
        action: String,
        /// The variable.
        variable: String,
    },
    /// An action specifies one state variable twice.
    ConflictingUpdate {
        /// The action.
        action: String,
        /// The variable.
        variable: String,
    },
    /// A primed or post-state form outside an action.
    PrimeOutsideAction,
    /// A temporal form (`always`, `eventually`, `~>`, `step`, `stutter`, the init
    /// name, `state`) outside a behavior.
    TemporalOutsideBehavior,
    /// A second `init` declaration.
    DuplicateInit,
    /// A name in a choice, `fairness`, or `step` that is not an action or choice.
    NotAnAction(String),
    /// An elaborated model beyond a hard size bound (INV-016: source is untrusted).
    TooLarge,
    /// Elaboration would exceed the work budget ([`crate::budget::MAX_WORK`]).
    WorkLimitExceeded,
    /// Semantics outside the Finite core fragment.
    Unsupported(Unsupported),
}

impl ElabErrorKind {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            ElabErrorKind::Parse(e) => e.code(),
            ElabErrorKind::DuplicateName(_) => "cml.elab.duplicate_name",
            ElabErrorKind::UnknownName(_) => "cml.elab.unknown_name",
            ElabErrorKind::UnknownType(_) => "cml.elab.unknown_type",
            ElabErrorKind::TypeArity { .. } => "cml.elab.type_arity",
            ElabErrorKind::CyclicTypeAlias(_) => "cml.elab.cyclic_type_alias",
            ElabErrorKind::TypeMismatch { .. } => "cml.elab.type_mismatch",
            ElabErrorKind::CannotInferType(_) => "cml.elab.cannot_infer_type",
            ElabErrorKind::NotAValue(_) => "cml.elab.not_a_value",
            ElabErrorKind::UnknownFunction(_) => "cml.elab.unknown_function",
            ElabErrorKind::Arity { .. } => "cml.elab.arity",
            ElabErrorKind::UnknownField(_) => "cml.elab.unknown_field",
            ElabErrorKind::UnspecifiedStateChange { .. } => "cml.elab.unspecified_state_change",
            ElabErrorKind::ConflictingUpdate { .. } => "cml.elab.conflicting_update",
            ElabErrorKind::PrimeOutsideAction => "cml.elab.prime_outside_action",
            ElabErrorKind::TemporalOutsideBehavior => "cml.elab.temporal_outside_behavior",
            ElabErrorKind::DuplicateInit => "cml.elab.duplicate_init",
            ElabErrorKind::NotAnAction(_) => "cml.elab.not_an_action",
            ElabErrorKind::TooLarge => "cml.limit.elaboration_too_large",
            ElabErrorKind::WorkLimitExceeded => "cml.limit.work_limit_exceeded",
            ElabErrorKind::Unsupported(u) => u.code(),
        }
    }
}

impl fmt::Display for ElabErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElabErrorKind::Parse(e) => write!(f, "{}", e.kind),
            ElabErrorKind::DuplicateName(n) => write!(f, "`{n}` is already declared"),
            ElabErrorKind::UnknownName(n) => write!(f, "`{n}` is not declared here"),
            ElabErrorKind::UnknownType(n) => write!(f, "type `{n}` is not declared"),
            ElabErrorKind::TypeArity {
                name,
                expected,
                found,
            } => write!(
                f,
                "type `{name}` takes {expected} argument(s), found {found}"
            ),
            ElabErrorKind::CyclicTypeAlias(n) => write!(f, "type alias `{n}` refers to itself"),
            ElabErrorKind::TypeMismatch { expected, found } => {
                write!(f, "expected type {expected}, found {found}")
            }
            ElabErrorKind::CannotInferType(what) => {
                write!(f, "the type of {what} is not determined by its uses")
            }
            ElabErrorKind::NotAValue(n) => write!(f, "`{n}` is not a value"),
            ElabErrorKind::UnknownFunction(n) => write!(f, "`{n}` is not a known function"),
            ElabErrorKind::Arity {
                name,
                expected,
                found,
            } => write!(f, "`{name}` takes {expected} argument(s), found {found}"),
            ElabErrorKind::UnknownField(n) => write!(f, "no field `{n}`"),
            ElabErrorKind::UnspecifiedStateChange { action, variable } => write!(
                f,
                "action `{action}` does not specify `{variable}`; write `next {variable} = …` \
                 or `unchanged {variable}`"
            ),
            ElabErrorKind::ConflictingUpdate { action, variable } => {
                write!(f, "action `{action}` specifies `{variable}` twice")
            }
            ElabErrorKind::PrimeOutsideAction => {
                write!(f, "a post-state value is allowed only in an action")
            }
            ElabErrorKind::TemporalOutsideBehavior => {
                write!(f, "a temporal form is allowed only in a behavior")
            }
            ElabErrorKind::DuplicateInit => write!(f, "a model has at most one init"),
            ElabErrorKind::NotAnAction(n) => write!(f, "`{n}` is not an action"),
            ElabErrorKind::TooLarge => write!(f, "the elaborated model exceeds the size bound"),
            ElabErrorKind::WorkLimitExceeded => {
                write!(f, "elaboration exceeds the work bound")
            }
            ElabErrorKind::Unsupported(u) => write!(f, "unsupported: {u}"),
        }
    }
}
