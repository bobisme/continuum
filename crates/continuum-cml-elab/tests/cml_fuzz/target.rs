//! The four targets: parse, elaborate, lower, and lower under a run configuration.
//!
//! Each target runs the real public entry point of the pipeline stage it names, under
//! [`FUZZ_LIMITS`], and reports a [`Probe`]. The later targets include the earlier
//! stages, so a refusal from an earlier stage is a landing of every later target too:
//! the codes are prefixed by stage (`cml.lex`, `cml.parse`, `cml.unsupported`,
//! `cml.limit`, `cml.elab`, `cml.lower`, `cml.config`), so the landing names the stage.
//!
//! # The round trip each target checks
//!
//! There is no CML formatter yet (docs/11 §14 keeps it for PR 15b, and it is defined
//! over the normalized AST). What exists is the parser's canonical printer
//! (`continuum_cml_syntax::print`), a round-trip witness that prints a tree back to
//! source. So the round trip here is the printer's, and it is checked at every stage:
//!
//! - parse: `print(parse(s))` re-parses to the same tree (the span-free dump), and the
//!   print is a fixpoint: printing the re-parse gives the same text;
//! - elaborate: `print(parse(s))` elaborates to the same normalized identity, or is
//!   refused with the same code;
//! - lower: `print(parse(s))` lowers to the same `Model::identity`, or is refused with
//!   the same code.

use std::sync::OnceLock;

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::lower_configured;
use continuum_cml_elab::{Limits, NormModel, Usage, elaborate_source_with, lower_with};
use continuum_cml_syntax::dump::dump_file;
use continuum_cml_syntax::print::print_file;
use continuum_cml_syntax::{Span, parse};

use super::depth::file_tree_depth;
use super::engine::{FUZZ_LIMITS, Probe, RoundTrip, Target, dump_nesting};

/// The deepest a parsed tree, expression or type, may be: the parser's own bound, measured exactly
/// by [`file_tree_depth`] (a leaf is depth 1).
pub const PARSE_TREE_DEPTH: usize = continuum_cml_syntax::MAX_NESTING as usize;

/// The deepest a normalized model's dump may nest: two dump levels per tree level (a
/// node, and a list that holds its children) over the elaborator's own tree bound
/// (`elab::MAX_TREE_DEPTH`, which adds inlined `let`s and `def`s to the parser's
/// nesting), and a few for the declaration around it. Coarser than the parse measure —
/// a factor of two — but linear in the bound, so a tree quadratic in it is caught.
pub const NORM_DUMP_DEPTH: usize = 2 * continuum_cml_elab::elab::MAX_TREE_DEPTH + 8;

/// The landing of a source every stage of a target accepted.
pub const ACCEPTED: &str = "accepted";

/// The landing of the configured target for a model no committed configuration names.
pub const UNCONFIGURED: &str = "unconfigured";

/// A 128-bit fingerprint of `bytes`: two FNV-1a passes with different offset bases,
/// as 32 hex digits. A test fingerprint, not a content address — the identities
/// themselves are the canonical encodings the crates define.
#[must_use]
pub fn fingerprint(bytes: &[u8]) -> String {
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut a: u64 = 0xcbf2_9ce4_8422_2325;
    let mut b: u64 = 0x6c62_272e_07bb_0142;
    for &byte in bytes {
        a = (a ^ u64::from(byte)).wrapping_mul(PRIME);
        b = (b ^ u64::from(byte.rotate_left(3))).wrapping_mul(PRIME);
    }
    let len = bytes.len() as u64;
    b ^= len;
    b = b.wrapping_mul(PRIME);
    format!("{a:016x}{b:016x}")
}

// --- the closed vocabularies ---------------------------------------------------------

/// Every code `continuum_cml_syntax::parse` can return, and `accepted`.
pub const PARSE: &[&str] = &[
    ACCEPTED,
    "cml.lex.unexpected_character",
    "cml.lex.unterminated_string",
    "cml.lex.invalid_escape",
    "cml.lex.integer_too_large",
    "cml.parse.unexpected_token",
    "cml.parse.chained_operator",
    "cml.parse.statement_not_allowed",
    "cml.limit.nesting_too_deep",
    "cml.limit.input_too_large",
    "cml.unsupported.parameterized_model",
    "cml.unsupported.parameterized_sort",
    "cml.unsupported.process",
    "cml.unsupported.await",
    "cml.unsupported.goto",
    "cml.unsupported.view",
    "cml.unsupported.forge_block",
    "cml.unsupported.synthesis_hole",
    "cml.unsupported.progress_property",
    "cml.unsupported.eventually_property",
    "cml.unsupported.transition_invariant",
    "cml.unsupported.hyperproperty",
    "cml.unsupported.symmetry",
    "cml.unsupported.check_directive",
    "cml.unsupported.module_import",
    "cml.unsupported.opaque_domain",
    "cml.unsupported.choose",
    "cml.unsupported.temporal_next",
    "cml.unsupported.float_literal",
];

/// Every code elaboration adds.
pub const ELAB_ONLY: &[&str] = &[
    "cml.elab.duplicate_name",
    "cml.elab.unknown_name",
    "cml.elab.unknown_type",
    "cml.elab.type_arity",
    "cml.elab.cyclic_type_alias",
    "cml.elab.type_mismatch",
    "cml.elab.cannot_infer_type",
    "cml.elab.not_a_value",
    "cml.elab.unknown_function",
    "cml.elab.arity",
    "cml.elab.unknown_field",
    "cml.elab.unspecified_state_change",
    "cml.elab.conflicting_update",
    "cml.elab.prime_outside_action",
    "cml.elab.temporal_outside_behavior",
    "cml.elab.duplicate_init",
    "cml.elab.not_an_action",
    "cml.elab.measure_out_of_domain",
    "cml.limit.elaboration_too_large",
    "cml.limit.work_limit_exceeded",
    "cml.elab.unsupported.mutual_recursion",
    "cml.elab.unsupported.no_decreasing_measure",
    "cml.elab.unsupported.recursion_bound_not_constant",
    "cml.elab.unsupported.primed_expression",
    "cml.elab.unsupported.integer_beyond_i64",
];

/// Every code lowering adds.
pub const LOWER_ONLY: &[&str] = &[
    "cml.lower.non_integer_state",
    "cml.lower.unbounded_domain",
    "cml.lower.non_interval_refinement",
    "cml.lower.parameterized_action",
    "cml.lower.constant",
    "cml.lower.quantifier",
    "cml.lower.division_or_modulo",
    "cml.lower.non_integer_value",
    "cml.lower.conditional_value",
    "cml.lower.non_standard_behavior",
    "cml.lower.no_init",
    "cml.lower.init_domain_too_large",
    "cml.lower.too_many_initial_states",
    "cml.lower.empty_refinement",
    "cml.lower.unbounded_type",
    "cml.lower.guarded_overflow",
    "cml.lower.recursive_call",
    "cml.lower.successor_domain_too_large",
    "cml.lower.too_many_variables",
    "cml.lower.too_many_actions",
    "cml.lower.name_too_long",
    "cml.lower.primed_outside_postcondition",
    "cml.lower.output_too_large",
    "cml.lower.work_limit_exceeded",
    "cml.lower.expression_too_deep",
    "cml.lower.dynamic_key",
    "cml.lower.value_outside_bound",
    "cml.lower.dynamic_value",
    "cml.lower.duplicate_key",
    "cml.lower.undefined_read",
    "cml.lower.model_refused",
    "cml.lower.evaluation",
];

/// Every code a run configuration adds, and the configured target's own landing.
pub const CONFIG_ONLY: &[&str] = &[
    UNCONFIGURED,
    "cml.config.other_model",
    "cml.config.missing_sort",
    "cml.config.extra_sort",
    "cml.config.missing_constant",
    "cml.config.extra_constant",
    "cml.config.ill_typed",
    "cml.config.unbindable_type",
    "cml.config.outside_bound",
];

fn union(parts: &[&'static [&'static str]]) -> &'static [&'static str] {
    let all: Vec<&'static str> = parts.iter().flat_map(|p| p.iter().copied()).collect();
    Box::leak(all.into_boxed_slice())
}

fn elab_vocabulary() -> &'static [&'static str] {
    static V: OnceLock<&'static [&'static str]> = OnceLock::new();
    V.get_or_init(|| union(&[PARSE, ELAB_ONLY]))
}

fn lower_vocabulary() -> &'static [&'static str] {
    static V: OnceLock<&'static [&'static str]> = OnceLock::new();
    V.get_or_init(|| union(&[PARSE, ELAB_ONLY, LOWER_ONLY]))
}

fn configured_vocabulary() -> &'static [&'static str] {
    static V: OnceLock<&'static [&'static str]> = OnceLock::new();
    V.get_or_init(|| union(&[PARSE, ELAB_ONLY, LOWER_ONLY, CONFIG_ONLY]))
}

// --- shared stages ---------------------------------------------------------------------

/// A refusal: its code, its display text, and its span.
struct Refusal {
    code: &'static str,
    text: String,
    span: Span,
}

fn refused(code: &'static str, text: String, span: Span, usage: Vec<(Limits, Usage)>) -> Probe {
    Probe {
        landing: code.to_owned(),
        detail: text,
        span: Some(span),
        usage,
        round_trip: RoundTrip::NotApplicable,
        depth: 0,
    }
}

/// The printed form of `src`, when it parses.
fn printed(src: &str) -> Option<String> {
    parse(src).ok().map(|file| print_file(&file))
}

/// Elaborate under [`FUZZ_LIMITS`], recording the usage.
fn elaborate(src: &str, usage: &mut Vec<(Limits, Usage)>) -> Result<NormModel, Refusal> {
    let (result, spent) = elaborate_source_with(src, FUZZ_LIMITS);
    usage.push((FUZZ_LIMITS, spent));
    result.map_err(|e| Refusal {
        code: e.code(),
        text: e.to_string(),
        span: e.span,
    })
}

/// A lowered identity, or a refusal.
type Lowered = Result<String, Refusal>;

fn lower_plain(model: &NormModel, usage: &mut Vec<(Limits, Usage)>) -> Lowered {
    let (result, spent) = lower_with(model, FUZZ_LIMITS);
    usage.push((FUZZ_LIMITS, spent));
    result
        .map(|m| fingerprint(m.identity().as_bytes()))
        .map_err(|e| Refusal {
            code: e.code(),
            text: e.to_string(),
            span: e.span,
        })
}

/// The committed run configurations, by the model name they bind. Read once.
fn configs() -> &'static [(String, RunConfig)] {
    static C: OnceLock<Vec<(String, RunConfig)>> = OnceLock::new();
    C.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        [
            root.join("tests/configs/ring.run-config.json"),
            root.join("../../notes/plan/schemas/examples/replicated-register.run-config.json"),
        ]
        .iter()
        .map(|path| {
            let bytes =
                std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let config = RunConfig::parse(&bytes)
                .unwrap_or_else(|e| panic!("{} reads: {e}", path.display()));
            (config.model().to_owned(), config)
        })
        .collect()
    })
}

/// Lower under the committed configuration for the model's name, if there is one.
fn lower_config(model: &NormModel, usage: &mut Vec<(Limits, Usage)>) -> Option<Lowered> {
    let (_, config) = configs().iter().find(|(name, _)| *name == model.name)?;
    let (result, spent) = lower_configured(model, config, FUZZ_LIMITS);
    usage.push((FUZZ_LIMITS, spent));
    Some(
        result
            .map(|c| {
                let mut both = c.model().identity().as_bytes().to_vec();
                both.extend_from_slice(&c.run_identity().encode());
                fingerprint(&both)
            })
            .map_err(|e| Refusal {
                code: e.code(),
                text: e.to_string(),
                span: e.span,
            }),
    )
}

/// Compare a stage's answer on the source with its answer on the printed form.
fn agree(
    stage: &str,
    source: &Result<String, &'static str>,
    reprint: &Result<String, &'static str>,
) -> RoundTrip {
    if source == reprint {
        RoundTrip::Held
    } else {
        RoundTrip::Broke(format!(
            "{stage}: the source gives {source:?}, its printed form gives {reprint:?}"
        ))
    }
}

// --- the targets -----------------------------------------------------------------------

/// `continuum_cml_syntax::parse`.
pub struct ParseTarget;

impl Target for ParseTarget {
    fn name(&self) -> &'static str {
        "cml.parse"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        PARSE
    }

    fn depth_limit(&self) -> usize {
        PARSE_TREE_DEPTH
    }

    fn probe(&self, src: &str) -> Probe {
        match parse(src) {
            Err(e) => refused(e.code(), e.to_string(), e.span, Vec::new()),
            Ok(file) => {
                // Measured first, without recursion: a tree past the bound is reported
                // as one before a recursive pass (dump, print) can overflow on it.
                let depth = file_tree_depth(&file);
                if depth > PARSE_TREE_DEPTH {
                    return Probe {
                        landing: ACCEPTED.to_owned(),
                        detail: format!("an expression tree {depth} deep"),
                        span: None,
                        usage: Vec::new(),
                        round_trip: RoundTrip::NotApplicable,
                        depth,
                    };
                }
                let dump = dump_file(&file);
                let text = print_file(&file);
                let round_trip = match parse(&text) {
                    Err(e) => RoundTrip::Broke(format!("the print does not re-parse: {e}")),
                    Ok(again) if dump_file(&again) != dump => {
                        RoundTrip::Broke("the print re-parses to another tree".to_owned())
                    }
                    Ok(again) if print_file(&again) != text => {
                        RoundTrip::Broke("the print is not a fixpoint".to_owned())
                    }
                    Ok(_) => RoundTrip::Held,
                };
                Probe {
                    landing: ACCEPTED.to_owned(),
                    detail: fingerprint(dump.as_bytes()),
                    span: None,
                    usage: Vec::new(),
                    round_trip,
                    depth,
                }
            }
        }
    }
}

/// `continuum_cml_elab::elaborate_source_with`.
pub struct ElabTarget;

fn elab_answer(src: &str, usage: &mut Vec<(Limits, Usage)>) -> Result<(String, usize), Refusal> {
    elaborate(src, usage).map(|m| {
        let dump = m.dump();
        let mut both = m.identity().as_bytes().to_vec();
        both.extend_from_slice(dump.as_bytes());
        (fingerprint(&both), dump_nesting(&dump))
    })
}

impl Target for ElabTarget {
    fn name(&self) -> &'static str {
        "cml.elab"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        elab_vocabulary()
    }

    fn depth_limit(&self) -> usize {
        NORM_DUMP_DEPTH
    }

    fn probe(&self, src: &str) -> Probe {
        let mut usage = Vec::new();
        let answer = elab_answer(src, &mut usage);
        let round_trip = match printed(src) {
            None => RoundTrip::NotApplicable,
            Some(text) => {
                let again = elab_answer(&text, &mut usage);
                agree(
                    "elaborate",
                    &answer.as_ref().map(|a| a.0.clone()).map_err(|r| r.code),
                    &again.map(|a| a.0).map_err(|r| r.code),
                )
            }
        };
        match answer {
            Err(r) => Probe {
                round_trip,
                ..refused(r.code, r.text, r.span, usage)
            },
            Ok((identity, depth)) => Probe {
                landing: ACCEPTED.to_owned(),
                detail: identity,
                span: None,
                usage,
                round_trip,
                depth,
            },
        }
    }
}

/// Elaborate then `continuum_cml_elab::lower_with`.
pub struct LowerTarget;

fn lower_answer(src: &str, usage: &mut Vec<(Limits, Usage)>) -> Lowered {
    let model = elaborate(src, usage)?;
    lower_plain(&model, usage)
}

impl Target for LowerTarget {
    fn name(&self) -> &'static str {
        "cml.lower"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        lower_vocabulary()
    }

    fn probe(&self, src: &str) -> Probe {
        let mut usage = Vec::new();
        let answer = lower_answer(src, &mut usage);
        let round_trip = match printed(src) {
            None => RoundTrip::NotApplicable,
            Some(text) => {
                let again = lower_answer(&text, &mut usage);
                agree(
                    "lower",
                    &answer.as_ref().map(Clone::clone).map_err(|r| r.code),
                    &again.map_err(|r| r.code),
                )
            }
        };
        match answer {
            Err(r) => Probe {
                round_trip,
                ..refused(r.code, r.text, r.span, usage)
            },
            Ok(identity) => Probe {
                landing: ACCEPTED.to_owned(),
                detail: identity,
                span: None,
                usage,
                round_trip,
                depth: 0,
            },
        }
    }
}

/// Elaborate then `continuum_cml_elab::lower::lower_configured`, under the committed
/// run configuration that names the model (Ring, the replicated register).
pub struct ConfiguredTarget;

fn configured_answer(src: &str, usage: &mut Vec<(Limits, Usage)>) -> Result<String, Refusal> {
    let model = elaborate(src, usage)?;
    match lower_config(&model, usage) {
        Some(answer) => answer,
        None => Err(Refusal {
            code: UNCONFIGURED,
            text: format!("no committed configuration names `{}`", model.name),
            // Not a source location: the harness's own landing carries no span.
            span: Span {
                start: 0,
                end: 0,
                line: 1,
                col: 1,
            },
        }),
    }
}

impl Target for ConfiguredTarget {
    fn name(&self) -> &'static str {
        "cml.lower-configured"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        configured_vocabulary()
    }

    fn probe(&self, src: &str) -> Probe {
        let mut usage = Vec::new();
        let answer = configured_answer(src, &mut usage);
        let round_trip = match printed(src) {
            None => RoundTrip::NotApplicable,
            Some(text) => {
                let again = configured_answer(&text, &mut usage);
                agree(
                    "lower under a configuration",
                    &answer.as_ref().map(Clone::clone).map_err(|r| r.code),
                    &again.map_err(|r| r.code),
                )
            }
        };
        match answer {
            Err(r) => Probe {
                round_trip,
                ..refused(r.code, r.text, r.span, usage)
            },
            Ok(identity) => Probe {
                landing: ACCEPTED.to_owned(),
                detail: identity,
                span: None,
                usage,
                round_trip,
                depth: 0,
            },
        }
    }
}

/// The four real targets, in pipeline order.
#[must_use]
pub fn all() -> [&'static dyn Target; 4] {
    [&ParseTarget, &ElabTarget, &LowerTarget, &ConfiguredTarget]
}

/// The real target called `name`.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static dyn Target> {
    all().into_iter().find(|t| t.name() == name)
}
