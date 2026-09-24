//! Deliberately defective targets, one per oracle.
//!
//! Each mutant is the real parse target with exactly one behaviour broken, and only on
//! its trigger source. `cml_fuzz.rs` asserts every mutant is caught by the oracle it
//! targets, and that the real target is clean on the same source, so what fires is the
//! injected defect and not the input.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use continuum_cml_elab::{Limits, Usage};
use continuum_cml_syntax::Span;

use continuum_cml_syntax::ast::{DeclKind, Ident, TypeExpr, TypeKind};

use super::depth::file_tree_depth;
use super::engine::{Budget, Oracle, Probe, RoundTrip, Target};

fn nat(span: Span) -> TypeExpr {
    TypeExpr {
        span,
        kind: TypeKind::Named(Ident {
            name: "Nat".to_owned(),
            span,
        }),
    }
}
use super::target::{PARSE, ParseTarget};

/// One injected defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defect {
    /// Panics on eight or more nested parentheses.
    PanicOnDeepNesting,
    /// Spins until released, past the wall budget.
    Spin,
    /// Answers differently on each call.
    Counter,
    /// Lands on a code outside the vocabulary.
    InventedCode,
    /// Reports a span past the end of the source.
    SpanPastEnd,
    /// Reports more work than its limits allow.
    Overspend,
    /// Claims the printed form disagrees.
    BrokenPrint,
    /// Reports an accepted tree deeper than its bound.
    Deep,
    /// Re-attaches a declared type's tree under new `Function` nodes past the bound,
    /// as the parser did before cr-3rqxh8, and reports it through the real measure.
    DeepType,
}

impl Defect {
    /// Every defect, one per oracle.
    pub const ALL: [Self; 9] = [
        Self::PanicOnDeepNesting,
        Self::Spin,
        Self::Counter,
        Self::InventedCode,
        Self::SpanPastEnd,
        Self::Overspend,
        Self::BrokenPrint,
        Self::Deep,
        Self::DeepType,
    ];

    /// The oracle that must catch it.
    #[must_use]
    pub const fn oracle(self) -> Oracle {
        match self {
            Self::PanicOnDeepNesting => Oracle::Panicked,
            Self::Spin => Oracle::Hung,
            Self::Counter => Oracle::Nondeterministic,
            Self::InventedCode => Oracle::Untyped,
            Self::SpanPastEnd => Oracle::Mislocated,
            Self::Overspend => Oracle::Overspent,
            Self::BrokenPrint => Oracle::RoundTrip,
            Self::Deep | Self::DeepType => Oracle::TooDeep,
        }
    }

    /// A source that trips the defect. Each is a real, small CML model the real target
    /// accepts, except the nesting trigger, which the real parser refuses typed.
    #[must_use]
    pub const fn trigger(self) -> &'static str {
        match self {
            Self::PanicOnDeepNesting => "module M\ninvariant I { ((((((((((x)))))))))) == 1 }\n",
            Self::DeepType => "module M\nconst C: Set[Nat] -> Nat\n",
            _ => "module M\nstate { x: Nat where x <= 1 }\ninit { x == 0 }\n",
        }
    }
}

/// The mutant target for one defect.
pub struct Mutant(pub Defect);

/// The mutants, as statics so they can be handed to a probe thread.
pub static MUTANTS: [Mutant; 9] = [
    Mutant(Defect::PanicOnDeepNesting),
    Mutant(Defect::Spin),
    Mutant(Defect::Counter),
    Mutant(Defect::InventedCode),
    Mutant(Defect::SpanPastEnd),
    Mutant(Defect::Overspend),
    Mutant(Defect::BrokenPrint),
    Mutant(Defect::Deep),
    Mutant(Defect::DeepType),
];

/// Set to release a spinning mutant after the harness has reported it hung.
pub static RELEASE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);

impl Target for Mutant {
    fn name(&self) -> &'static str {
        "mutant"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        PARSE
    }

    fn budget(&self) -> Budget {
        Budget {
            wall: Duration::from_millis(200),
            ..Budget::DEFAULT
        }
    }

    fn depth_limit(&self) -> usize {
        ParseTarget.depth_limit()
    }

    fn probe(&self, src: &str) -> Probe {
        let mut probe = ParseTarget.probe(src);
        match self.0 {
            Defect::PanicOnDeepNesting => {
                if src.contains("((((((((") {
                    panic!("mutant: unbounded recursion stands in here");
                }
            }
            Defect::Spin => {
                // Bounded by the release flag and, as a last resort, by a minute, so a
                // failed test never leaves a thread spinning for ever.
                let start = std::time::Instant::now();
                while !RELEASE.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(60) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            Defect::Counter => {
                probe.detail = CALLS.fetch_add(1, Ordering::SeqCst).to_string();
            }
            Defect::InventedCode => probe.landing = "cml.fuzz.invented".to_owned(),
            Defect::SpanPastEnd => {
                let end = u32::try_from(src.len())
                    .unwrap_or(u32::MAX)
                    .saturating_add(1);
                probe.span = Some(Span {
                    start: 0,
                    end,
                    line: 1,
                    col: 1,
                });
            }
            Defect::Overspend => {
                let limits = Limits { nodes: 8, work: 8 };
                probe.usage.push((limits, Usage { nodes: 8, work: 9 }));
            }
            Defect::Deep => probe.depth = ParseTarget.depth_limit() + 1,
            Defect::DeepType => {
                if let Ok(mut file) = continuum_cml_syntax::parse(src) {
                    for decl in &mut file.decls {
                        if let DeclKind::Const { ty, .. } = &mut decl.kind {
                            // The vulnerable shape: the parsed type re-attached, again
                            // and again, as the left side of a new function type.
                            for _ in 0..ParseTarget.depth_limit() {
                                let left = std::mem::replace(ty, nat(ty.span));
                                *ty = TypeExpr {
                                    span: left.span,
                                    kind: TypeKind::Function(
                                        Box::new(left),
                                        Box::new(nat(ty.span)),
                                    ),
                                };
                            }
                        }
                    }
                    probe.depth = file_tree_depth(&file);
                }
            }
            Defect::BrokenPrint => {
                probe.round_trip = RoundTrip::Broke("mutant: the print drops a clause".to_owned());
            }
        }
        probe
    }
}
