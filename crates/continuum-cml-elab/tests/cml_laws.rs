//! The three laws of the CML front-end pipeline, as property suites (bn-1nmq).
//!
//! | Law | Tests |
//! |---|---|
//! | (a) parse → print → parse is the identity; there is no formatter yet, so the printer is the pinned witness | [`law_a_print_then_parse_is_the_identity_on_every_corpus_entry`], [`law_a_the_formatter_is_absent_so_the_printer_is_the_witness`] |
//! | (b) elaboration is deterministic: one source, one semantic identity, whatever the layout, the thread, or the process | [`law_b_layout_does_not_change_the_identity`], [`law_b_concurrent_elaboration_agrees_with_sequential`], and in `cml_fuzz.rs` `identities_are_reproduced_by_independent_processes` and the manifest |
//! | (c) out-of-fragment rejection is total: every out-of-fragment construct, anywhere a declaration may stand, is a typed refusal naming that construct, never a panic and never an acceptance | [`law_c_every_syntactic_out_of_fragment_construct_is_refused_everywhere`], [`law_c_every_semantic_out_of_fragment_construct_is_refused_everywhere`], [`law_c_a_parameterized_header_is_refused_in_both_styles`] |
//!
//! The hosts are the fuzz corpus's seeds and hand-written cases
//! (`tests/cml-fuzz-corpus/`), so every law runs over every repository CML source and
//! every adversarial shape the corpus keeps. Each law is checked exhaustively over its
//! insertion points, not sampled.

#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test bodies assert on fixtures of known shape"
)]

#[allow(
    dead_code,
    reason = "the laws share the fuzz harness's corpus loader, not its generator or manifest"
)]
mod cml_fuzz {
    pub mod corpus;
    pub mod generate;
}

use std::thread;

use cml_fuzz::corpus::{self, Entry, Origin};
use continuum_cml_elab::{
    ElabErrorKind, NormModel, Unsupported as ElabUnsupported, elaborate_source, lower,
};
use continuum_cml_syntax::dump::dump_file;
use continuum_cml_syntax::print::print_file;
use continuum_cml_syntax::{ParseErrorKind, SourceFile, Unsupported, parse};

/// The seeds and the hand-written cases: the hosts every law runs over.
fn hosts() -> Vec<Entry> {
    corpus::all()
        .into_iter()
        .filter(|e| matches!(e.origin, Origin::Seed | Origin::Hand))
        .collect()
}

fn parsed_hosts() -> Vec<(Entry, SourceFile)> {
    hosts()
        .into_iter()
        .filter_map(|e| parse(&e.source).ok().map(|f| (e, f)))
        .collect()
}

fn elaborated_hosts() -> Vec<(Entry, NormModel)> {
    hosts()
        .into_iter()
        .filter_map(|e| elaborate_source(&e.source).ok().map(|m| (e, m)))
        .collect()
}

/// The byte offsets where a declaration may stand: before each declaration, and after
/// the last one.
fn boundaries(src: &str, file: &SourceFile) -> Vec<usize> {
    let mut at: Vec<usize> = file.decls.iter().map(|d| d.span.start as usize).collect();
    if let Some(last) = file.decls.last() {
        // After the last declaration, at the start of the next line.
        let end = last.span.end as usize;
        let next_line = src[end..].find('\n').map_or(src.len(), |i| end + i + 1);
        at.push(next_line);
    }
    at
}

fn splice(src: &str, at: usize, text: &str) -> String {
    format!("{}{}\n{}", &src[..at], text, &src[at..])
}

// --- (a) the round trip ------------------------------------------------------------------------

/// Law (a), docs/19 §3's "serialization round trip" relation on the CML surface: print
/// then parse is the identity on the tree, printing is a fixpoint, and the printed form
/// elaborates to the same normalized identity, for every corpus source that parses. The
/// fuzz campaign checks the same on every mutant it accepts (`cml_fuzz`
/// `Oracle::RoundTrip`), at every stage of the pipeline.
#[test]
fn law_a_print_then_parse_is_the_identity_on_every_corpus_entry() {
    let mut checked = 0;
    for e in corpus::all() {
        let Ok(file) = parse(&e.source) else { continue };
        let printed = print_file(&file);
        let again = parse(&printed).unwrap_or_else(|err| {
            panic!("{}: the print does not re-parse: {err}\n{printed}", e.name)
        });
        assert_eq!(dump_file(&again), dump_file(&file), "{}", e.name);
        assert_eq!(
            print_file(&again),
            printed,
            "{}: print is not a fixpoint",
            e.name
        );
        if let Ok(model) = elaborate_source(&e.source) {
            let reprinted = elaborate_source(&printed).expect("the print elaborates");
            assert_eq!(reprinted.identity(), model.identity(), "{}", e.name);
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} corpus entries parse");
}

/// Law (a) is pinned on the printer because there is no formatter: docs/11 §14 keeps the
/// deterministic formatter for PR 15b, defined over the normalized AST. This test is the
/// tripwire: when a formatter module lands in either front-end crate, it fails, so the
/// formatter's own round trip (format → parse → elaborate is the same identity, and
/// format is idempotent) gets added here rather than assumed.
#[test]
fn law_a_the_formatter_is_absent_so_the_printer_is_the_witness() {
    for krate in ["continuum-cml-syntax", "continuum-cml-elab"] {
        let dir = corpus::repo().join("crates").join(krate).join("src");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("a src directory")
            .map(|e| e.expect("an entry").path())
            .collect();
        files.sort();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(
                !name.starts_with("format") && !name.starts_with("fmt"),
                "{krate}/src/{name} looks like the PR 15b formatter: extend law (a) to it"
            );
        }
        let lib = std::fs::read_to_string(dir.join("lib.rs")).expect("lib.rs");
        assert!(
            !lib.contains("pub mod format") && !lib.contains("pub fn format"),
            "{krate} exports a formatter: extend law (a) to it"
        );
    }
}

// --- (b) determinism ----------------------------------------------------------------------------

/// Law (b): the identity does not depend on layout. Blank lines, `//` comments, and
/// trailing spaces at every declaration boundary of every host leave the normalized
/// identity, and, where the host lowers, the programmatic `Model::identity`, unchanged.
#[test]
fn law_b_layout_does_not_change_the_identity() {
    const LAYOUTS: [&str; 3] = ["", "// a comment, with { braces } and \"quotes\"", "   \t"];
    let mut checked = 0;
    for (e, model) in elaborated_hosts() {
        let identity = model.identity();
        let lowered = lower(&model).ok().map(|m| m.identity());
        let file = parse(&e.source).expect("an elaborated host parses");
        for at in boundaries(&e.source, &file) {
            for layout in LAYOUTS {
                let src = splice(&e.source, at, layout);
                let again = elaborate_source(&src)
                    .unwrap_or_else(|err| panic!("{} at {at}: {err}", e.name));
                assert_eq!(
                    again.identity(),
                    identity,
                    "{} at {at} with {layout:?}",
                    e.name
                );
                if let Some(want) = &lowered {
                    let got = lower(&again).expect("lowers again").identity();
                    assert_eq!(&got, want, "{} at {at}", e.name);
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 100, "only {checked} layouts checked");
}

/// Law (b): elaborating on several threads at once gives the same identities as one
/// thread, so no identity reads shared or ambient state.
#[test]
fn law_b_concurrent_elaboration_agrees_with_sequential() {
    let sources: Vec<String> = elaborated_hosts()
        .into_iter()
        .map(|(e, _)| e.source)
        .collect();
    let sequential: Vec<Vec<u8>> = sources
        .iter()
        .map(|s| {
            elaborate_source(s)
                .expect("elaborates")
                .identity()
                .as_bytes()
                .to_vec()
        })
        .collect();
    let workers: Vec<_> = (0..4)
        .map(|k| {
            let mine = sources.clone();
            thread::spawn(move || {
                // Each worker walks the sources from a different start.
                let n = mine.len();
                (0..n)
                    .map(|i| {
                        let j = (i + k * 3) % n;
                        let id = elaborate_source(&mine[j]).expect("elaborates").identity();
                        (j, id.as_bytes().to_vec())
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for w in workers {
        for (j, id) in w.join().expect("a worker") {
            assert_eq!(id, sequential[j], "source {j} differs on another thread");
        }
    }
    assert!(sequential.len() >= 10);
}

// --- (c) out-of-fragment totality -----------------------------------------------------------

/// One declaration per syntactic out-of-fragment construct, as a declaration or inside
/// one. The `match` makes a new `Unsupported` variant a compile error here until it has a
/// snippet.
fn syntax_snippet(u: Unsupported) -> Option<&'static str> {
    Some(match u {
        // A header, not a declaration: see the header law below.
        Unsupported::ParameterizedModel => return None,
        Unsupported::ParameterizedSort => "const OofXs: Set[OofNode<3>]",
        Unsupported::Process => "process OofP(p in OofNodes) {\n}",
        Unsupported::Await => "action OofA {\n  await oof_ready\n}",
        Unsupported::Goto => "action OofG {\n  goto OofDone\n}",
        Unsupported::View => "view OofV from OofP {\n}",
        Unsupported::ForgeBlock => "forge {\n}",
        Unsupported::SynthesisHole => "invariant OofH { ??guard(oof) }",
        Unsupported::ProgressProperty => "progress OofProgress {\n  true\n}",
        Unsupported::EventuallyProperty => "eventually OofEventually {\n  true\n}",
        Unsupported::TransitionInvariant => "transition invariant OofT {\n  true\n}",
        Unsupported::Hyperproperty => "hyperproperty OofHyper {\n  true\n}",
        Unsupported::Symmetry => "symmetry rotate(OofNode)",
        Unsupported::CheckDirective => "check deadlock",
        Unsupported::ModuleImport => "import OofPaxos",
        Unsupported::OpaqueDomain => "extern type OofClock",
        Unsupported::Choose => "action OofC {\n  let n = choose OofNode where oof[n]\n}",
        Unsupported::TemporalNext => "invariant OofN { next(oof) == oof }",
        Unsupported::FloatLiteral => "invariant OofF { oof < 1.5 }",
    })
}

/// Law (c), syntactic half: each construct in `continuum_cml_syntax::Unsupported::ALL`,
/// spliced at every declaration boundary of every host that parses, is refused with
/// exactly its own code, at a span inside the spliced text. Never an acceptance, never
/// another error first.
#[test]
fn law_c_every_syntactic_out_of_fragment_construct_is_refused_everywhere() {
    let hosts = parsed_hosts();
    assert!(hosts.len() >= 10, "only {} hosts parse", hosts.len());
    let mut checked = 0;
    for u in Unsupported::ALL {
        let Some(snippet) = syntax_snippet(u) else {
            continue;
        };
        for (e, file) in &hosts {
            for at in boundaries(&e.source, file) {
                let src = splice(&e.source, at, snippet);
                let err = match parse(&src) {
                    Ok(_) => panic!("{u:?} spliced into {} at {at} was accepted", e.name),
                    Err(err) => err,
                };
                assert_eq!(
                    err.kind,
                    ParseErrorKind::Unsupported(u),
                    "{} at {at}: {err}",
                    e.name
                );
                let start = err.span.start as usize;
                assert!(
                    (at..at + snippet.len()).contains(&start),
                    "{} at {at}: {u:?} located at {start}, outside the construct",
                    e.name
                );
                // The elaborator reports the parse refusal unchanged.
                let elab = elaborate_source(&src).expect_err("refused");
                assert!(elab.is_unsupported() && elab.code() == u.code());
                checked += 1;
            }
        }
    }
    assert!(checked >= 500, "only {checked} splices checked");
}

/// Law (c): a parameterized model header is refused in both header styles.
#[test]
fn law_c_a_parameterized_header_is_refused_in_both_styles() {
    let mut checked = 0;
    for (e, file) in parsed_hosts() {
        let name = &file.header.name;
        let end = name.span.end as usize;
        let src = format!("{}<const N: Nat>{}", &e.source[..end], &e.source[end..]);
        let err = parse(&src).expect_err("a parameterized header is refused");
        assert_eq!(
            err.kind,
            ParseErrorKind::Unsupported(Unsupported::ParameterizedModel),
            "{}: {err}",
            e.name
        );
        checked += 1;
    }
    assert!(checked >= 10);
}

/// One declaration per semantic out-of-fragment construct: well-formed syntax the
/// elaborator refuses as outside the Finite core, and that construct's position in
/// [`ELAB_UNSUPPORTED`]. The `match` makes a new variant a compile error here until it
/// has a snippet and a position, and the position is the reminder to add it to the
/// table. That last step is not mechanical: `continuum_cml_elab::Unsupported` has no
/// `ALL` constant (the syntax crate's has one), so the table test checks order and
/// uniqueness, not completeness.
fn elab_snippet(u: ElabUnsupported) -> (usize, &'static str) {
    match u {
        ElabUnsupported::MutualRecursion => (
            0,
            "def oof_even(n: Nat): Bool = if n == 0 then true else oof_odd(n - 1)\n\
             def oof_odd(n: Nat): Bool = if n == 0 then false else oof_even(n - 1)",
        ),
        ElabUnsupported::NoDecreasingMeasure => (1, "def oof_spin(n: Nat): Nat = oof_spin(n + 1)"),
        ElabUnsupported::RecursionBoundNotConstant => (
            2,
            "def oof_down(n: Nat): Nat = if n == 0 then 0 else oof_down(n - 1)\n\
             def oof_call(m: Nat): Nat = oof_down(m)",
        ),
        ElabUnsupported::PrimedExpression => (3, "action OofPrimed {\n  require (0 + 1)' == 1\n}"),
        ElabUnsupported::IntegerBeyondI64 => {
            (4, "def oof_big(n: Nat): Bool = n < 9223372036854775808")
        }
    }
}

/// Every [`ElabUnsupported`] variant, at the position [`elab_snippet`] gives it.
const ELAB_UNSUPPORTED: [ElabUnsupported; 5] = [
    ElabUnsupported::MutualRecursion,
    ElabUnsupported::NoDecreasingMeasure,
    ElabUnsupported::RecursionBoundNotConstant,
    ElabUnsupported::PrimedExpression,
    ElabUnsupported::IntegerBeyondI64,
];

/// The table is in `elab_snippet`'s order, with no variant listed twice.
#[test]
fn law_c_the_semantic_construct_table_is_exhaustive() {
    for (i, u) in ELAB_UNSUPPORTED.iter().enumerate() {
        assert_eq!(elab_snippet(*u).0, i, "{u:?} is out of place");
    }
    let mut codes: Vec<&str> = ELAB_UNSUPPORTED.iter().map(|u| u.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(
        codes.len(),
        ELAB_UNSUPPORTED.len(),
        "a variant is listed twice"
    );
}

/// Law (c), semantic half: each construct in [`ElabUnsupported`], spliced at every
/// declaration boundary of every host that elaborates, is refused, typed as out of
/// fragment, with exactly its own code. Never an acceptance, never another error.
#[test]
fn law_c_every_semantic_out_of_fragment_construct_is_refused_everywhere() {
    let hosts = elaborated_hosts();
    assert!(hosts.len() >= 10, "only {} hosts elaborate", hosts.len());
    let mut checked = 0;
    for u in ELAB_UNSUPPORTED {
        let (_, snippet) = elab_snippet(u);
        for (e, _) in &hosts {
            let file = parse(&e.source).expect("parses");
            for at in boundaries(&e.source, &file) {
                let src = splice(&e.source, at, snippet);
                let err = match elaborate_source(&src) {
                    Ok(_) => panic!("{u:?} spliced into {} at {at} was accepted", e.name),
                    Err(err) => err,
                };
                assert_eq!(
                    err.kind,
                    ElabErrorKind::Unsupported(u),
                    "{} at {at}: {err}",
                    e.name
                );
                assert!(err.is_unsupported());
                let start = err.span.start as usize;
                assert!(
                    (at..at + snippet.len()).contains(&start),
                    "{} at {at}: {u:?} located at {start}, outside the construct",
                    e.name
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 200, "only {checked} splices checked");
}
