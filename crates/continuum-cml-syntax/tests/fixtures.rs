//! Acceptance fixtures for the CML Finite core parser (PR 15a, bn-1sf).
//!
//! # What is pinned
//!
//! | Fixture | Source | Expectation |
//! |---|---|---|
//! | replicated register | `notes/plan/examples/replicated_register.ctm` | parses; golden tree |
//! | Die Hard (TV-009, Wave 0) | `notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm` | parses; golden tree |
//! | Coffee Can, Tower of Hanoi, Transitive Closure | `tests/fixtures/` (Wave 0 anchor shapes) | parse; golden trees |
//! | syntax coverage | `tests/fixtures/SyntaxCoverage.ctm` | parses; golden tree |
//! | Dining Philosophers (TV-007, Wave 1) | `notes/plan/corpus/…/TV-007/DiningPhilosophers.ctm` | typed unsupported, with span |
//! | Forge ack protocol | `notes/plan/examples/forge_ack_protocol.ctm` | typed unsupported, with span |
//!
//! The dossier fixtures are read in place, so a change to the dossier source is a change
//! to this test's input.
//!
//! Every accepted fixture also passes the canonical round trip: printing the tree and
//! re-parsing the print gives the same tree, and printing is idempotent.
//!
//! Regenerate the golden trees with `CML_BLESS=1 cargo test -p continuum-cml-syntax
//! --test fixtures`, and review the diff.

use std::path::PathBuf;

use continuum_cml_syntax::dump::dump_file;
use continuum_cml_syntax::print::print_file;
use continuum_cml_syntax::{ParseErrorKind, Unsupported, parse};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dossier(rel: &str) -> PathBuf {
    manifest().join("../../notes/plan").join(rel)
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Parse `path`, compare its tree dump with `tests/golden/<name>.tree`, and check the
/// canonical round trip.
fn accept(name: &str, path: &PathBuf) {
    let src = read(path);
    let file = parse(&src).unwrap_or_else(|e| panic!("{name} must parse: {e}"));
    let dump = dump_file(&file);

    let golden = manifest().join("tests/golden").join(format!("{name}.tree"));
    if std::env::var_os("CML_BLESS").is_some() {
        std::fs::write(&golden, &dump).expect("write golden");
    }
    let expected = read(&golden);
    assert_eq!(
        dump,
        expected,
        "{name}: tree differs from {}",
        golden.display()
    );

    // Determinism: a second parse of the same text gives the identical tree, spans included.
    assert_eq!(
        parse(&src).expect("second parse"),
        file,
        "{name}: parse is not deterministic"
    );

    // Canonical round trip.
    let printed = print_file(&file);
    let reparsed = parse(&printed)
        .unwrap_or_else(|e| panic!("{name}: canonical print does not re-parse: {e}\n{printed}"));
    assert_eq!(
        dump_file(&reparsed),
        dump,
        "{name}: print/parse changed the tree"
    );
    assert_eq!(
        print_file(&reparsed),
        printed,
        "{name}: printing is not idempotent"
    );
}

#[test]
fn positive_replicated_register_parses_to_its_golden_tree() {
    accept(
        "replicated_register",
        &dossier("examples/replicated_register.ctm"),
    );
}

#[test]
fn positive_die_hard_wave0_port_parses_to_its_golden_tree() {
    accept(
        "DieHard",
        &dossier("corpus/tla-examples/ports/TV-009/DieHard.ctm"),
    );
}

#[test]
fn positive_wave0_shape_fixtures_parse_to_their_golden_trees() {
    for name in ["CoffeeCan", "TowerOfHanoi", "TransitiveClosure"] {
        accept(name, &manifest().join(format!("tests/fixtures/{name}.ctm")));
    }
}

#[test]
fn positive_syntax_coverage_fixture_parses_to_its_golden_tree() {
    accept(
        "SyntaxCoverage",
        &manifest().join("tests/fixtures/SyntaxCoverage.ctm"),
    );
}

/// The span's source text.
fn slice(src: &str, span: continuum_cml_syntax::Span) -> &str {
    &src[span.start as usize..span.end as usize]
}

#[test]
fn negative_dining_philosophers_is_typed_unsupported_not_accepted() {
    let src = read(&dossier(
        "corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm",
    ));
    let err = parse(&src).expect_err("a Wave 1 parameterized, procedural model must not parse");
    assert_eq!(
        err.kind,
        ParseErrorKind::Unsupported(Unsupported::ParameterizedModel)
    );
    assert_eq!((err.span.line, err.span.col), (2, 25));
    assert_eq!(slice(&src, err.span), "<");
    assert_eq!(err.code(), "cml.unsupported.parameterized_model");
}

#[test]
fn negative_forge_holes_are_typed_unsupported_not_accepted() {
    let src = read(&dossier("examples/forge_ack_protocol.ctm"));
    let err = parse(&src).expect_err("a Forge hole must not parse");
    assert_eq!(
        err.kind,
        ParseErrorKind::Unsupported(Unsupported::SynthesisHole)
    );
    assert_eq!((err.span.line, err.span.col), (24, 13));
    assert_eq!(slice(&src, err.span), "??ack_guard");
}

#[test]
fn positive_spans_locate_the_replicated_register_declarations() {
    use continuum_cml_syntax::ast::DeclKind;
    let src = read(&dossier("examples/replicated_register.ctm"));
    let file = parse(&src).expect("parses");
    assert_eq!(slice(&src, file.header.span), "module ReplicatedRegister");
    let choose = file
        .decls
        .iter()
        .find_map(|d| match &d.kind {
            DeclKind::Action { name, .. } if name.name == "Choose" => Some((d, name)),
            _ => None,
        })
        .expect("action Choose");
    assert_eq!((choose.1.span.line, choose.1.span.col), (28, 8));
    assert!(slice(&src, choose.0.span).starts_with("action Choose(epoch: Nat"));
    assert!(slice(&src, choose.0.span).ends_with("unchanged stable, alive\n}"));
    let last = file.decls.last().expect("decls");
    assert_eq!(slice(&src, last.span), "fairness weak Recover");
    assert_eq!(slice(&src, file.span).len(), src.trim_end().len());
}
