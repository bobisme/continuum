//! Seeded property and fuzz tests for the CML Finite core parser (PR 15a, bn-1sf).
//!
//! Every run draws from fixed seeds, so a failure names its seed and case index and
//! replays exactly. No ambient randomness is used (INV-005).
//!
//! | Claim | Test |
//! |---|---|
//! | print then parse is the identity on generated expression trees | [`property_print_parse_round_trips_generated_expressions`] |
//! | print then parse is the identity on generated types | [`property_print_parse_round_trips_generated_types`] |
//! | mutated fixtures never panic; every error span is in bounds and located right; every accepted mutant round-trips | [`fuzz_mutated_fixtures_fail_typed_or_round_trip`] |

use continuum_cml_syntax::ast::{
    BinOp, Binder, Expr, ExprKind, Ident, Quantifier, TemporalOp, TypeExpr, TypeKind, UnOp,
};
use continuum_cml_syntax::dump::{dump_expr, dump_file, dump_type};
use continuum_cml_syntax::print::{print_expr, print_file, print_type};
use continuum_cml_syntax::{Span, parse, parse_expr, parse_type};

/// SplitMix64: a small, fully specified generator, so a seed means the same draw on every
/// platform and release.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("fits")).expect("fits")
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const SPAN: Span = Span {
    start: 0,
    end: 0,
    line: 1,
    col: 1,
};

const NAMES: [&str; 8] = ["x", "y", "alive", "Nodes", "v1", "_t", "process", "Some"];

const BINOPS: [BinOp; 23] = [
    BinOp::LeadsTo,
    BinOp::Iff,
    BinOp::Implies,
    BinOp::Or,
    BinOp::And,
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Le,
    BinOp::Gt,
    BinOp::Ge,
    BinOp::In,
    BinOp::NotIn,
    BinOp::SubsetEq,
    BinOp::Range,
    BinOp::Union,
    BinOp::Intersect,
    BinOp::Diff,
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::Div,
    BinOp::Mod,
];

fn ident(rng: &mut Rng) -> Ident {
    Ident {
        name: (*rng.pick(&NAMES)).to_owned(),
        span: SPAN,
    }
}

fn e(kind: ExprKind) -> Expr {
    Expr { kind, span: SPAN }
}

fn bx(x: Expr) -> Box<Expr> {
    Box::new(x)
}

fn exprs(rng: &mut Rng, depth: u32, min: usize) -> Vec<Expr> {
    let n = min + rng.below(3);
    (0..n).map(|_| gen_expr(rng, depth)).collect()
}

fn binders(rng: &mut Rng, depth: u32) -> Vec<Binder> {
    let n = 1 + rng.below(2);
    (0..n)
        .map(|_| Binder {
            name: ident(rng),
            domain: (rng.below(2) == 0).then(|| gen_expr(rng, depth)),
            span: SPAN,
        })
        .collect()
}

fn gen_str(rng: &mut Rng) -> String {
    let alphabet = ['a', 'Z', ' ', '"', '\\', '\n', 'é', '0', '{'];
    (0..rng.below(5)).map(|_| *rng.pick(&alphabet)).collect()
}

/// A random expression tree of at most `depth` further levels.
fn gen_expr(rng: &mut Rng, depth: u32) -> Expr {
    let leaf = depth == 0 || rng.below(4) == 0;
    if leaf {
        return match rng.below(5) {
            0 => e(ExprKind::Int(rng.next() >> rng.below(64))),
            1 => e(ExprKind::Bool(rng.below(2) == 0)),
            2 => e(ExprKind::Str(gen_str(rng))),
            3 => e(ExprKind::WholeState),
            _ => e(ExprKind::Name(ident(rng))),
        };
    }
    let d = depth - 1;
    match rng.below(21) {
        0 => e(ExprKind::Prime(bx(gen_expr(rng, d)))),
        1 => e(ExprKind::Unary(
            *rng.pick(&[UnOp::Not, UnOp::Neg]),
            bx(gen_expr(rng, d)),
        )),
        2..=5 => e(ExprKind::Binary(
            *rng.pick(&BINOPS),
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
        )),
        6 => e(ExprKind::Quant(
            *rng.pick(&[Quantifier::Forall, Quantifier::Exists]),
            binders(rng, d),
            bx(gen_expr(rng, d)),
        )),
        7 => e(ExprKind::If(
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
        )),
        8 => e(ExprKind::Temporal(
            *rng.pick(&[TemporalOp::Always, TemporalOp::Eventually]),
            bx(gen_expr(rng, d)),
        )),
        9 => e(ExprKind::Call(ident(rng), exprs(rng, d, 0))),
        10 => e(ExprKind::Method(
            bx(gen_expr(rng, d)),
            ident(rng),
            exprs(rng, d, 0),
        )),
        11 => e(ExprKind::Field(bx(gen_expr(rng, d)), ident(rng))),
        12 => e(ExprKind::Index(bx(gen_expr(rng, d)), bx(gen_expr(rng, d)))),
        13 => e(ExprKind::Update(
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
        )),
        14 => e(ExprKind::Tuple(exprs(rng, d, 2))),
        15 => e(if rng.below(3) == 0 {
            ExprKind::EmptyBraces
        } else {
            ExprKind::SetLit(exprs(rng, d, 1))
        }),
        16 => e(ExprKind::MapLit(
            (0..1 + rng.below(2))
                .map(|_| (gen_expr(rng, d), gen_expr(rng, d)))
                .collect(),
        )),
        17 => e(ExprKind::SetComp(
            bx(gen_expr(rng, d)),
            binders(rng, d),
            (rng.below(2) == 0).then(|| bx(gen_expr(rng, d))),
        )),
        18 => e(ExprKind::MapComp(
            bx(gen_expr(rng, d)),
            bx(gen_expr(rng, d)),
            binders(rng, d),
            (rng.below(2) == 0).then(|| bx(gen_expr(rng, d))),
        )),
        19 => e(ExprKind::Record(
            (0..1 + rng.below(2))
                .map(|_| (ident(rng), gen_expr(rng, d)))
                .collect(),
        )),
        _ => e(ExprKind::SeqLit(exprs(rng, d, 0))),
    }
}

fn gen_type(rng: &mut Rng, depth: u32) -> TypeExpr {
    let t = |kind| TypeExpr { kind, span: SPAN };
    if depth == 0 || rng.below(3) == 0 {
        return t(TypeKind::Named(ident(rng)));
    }
    let d = depth - 1;
    match rng.below(4) {
        0 => t(TypeKind::Applied(
            ident(rng),
            (0..1 + rng.below(2)).map(|_| gen_type(rng, d)).collect(),
        )),
        1 => t(TypeKind::Tuple(
            (0..2 + rng.below(2)).map(|_| gen_type(rng, d)).collect(),
        )),
        2 => t(TypeKind::Function(
            Box::new(gen_type(rng, d)),
            Box::new(gen_type(rng, d)),
        )),
        _ => t(TypeKind::Record(
            (0..1 + rng.below(2))
                .map(|_| (ident(rng), gen_type(rng, d)))
                .collect(),
        )),
    }
}

/// The expression form, for coverage accounting.
fn form(k: &ExprKind) -> &'static str {
    match k {
        ExprKind::Int(_) => "int",
        ExprKind::Bool(_) => "bool",
        ExprKind::Str(_) => "str",
        ExprKind::Name(_) => "name",
        ExprKind::WholeState => "state",
        ExprKind::Prime(_) => "prime",
        ExprKind::Unary(..) => "unary",
        ExprKind::Binary(..) => "binary",
        ExprKind::Quant(..) => "quant",
        ExprKind::If(..) => "if",
        ExprKind::Temporal(..) => "temporal",
        ExprKind::Call(..) => "call",
        ExprKind::Method(..) => "method",
        ExprKind::Field(..) => "field",
        ExprKind::Index(..) => "index",
        ExprKind::Update(..) => "update",
        ExprKind::Tuple(_) => "tuple",
        ExprKind::EmptyBraces => "empty",
        ExprKind::SetLit(_) => "set",
        ExprKind::MapLit(_) => "map",
        ExprKind::SetComp(..) => "set-comp",
        ExprKind::MapComp(..) => "map-comp",
        ExprKind::Record(_) => "record",
        ExprKind::SeqLit(_) => "seq",
    }
}

#[test]
fn property_print_parse_round_trips_generated_expressions() {
    const SEED: u64 = 0x00C0_FFEE_15A0_0001;
    let mut rng = Rng(SEED);
    let mut shapes = std::collections::BTreeSet::new();
    for case in 0..4000 {
        let tree = gen_expr(&mut rng, 5);
        let printed = print_expr(&tree);
        let reparsed = parse_expr(&printed).unwrap_or_else(|err| {
            panic!("seed {SEED:#x} case {case}: print does not re-parse: {err}\n{printed}")
        });
        assert_eq!(
            dump_expr(&reparsed),
            dump_expr(&tree),
            "seed {SEED:#x} case {case}: print/parse changed the tree\n{printed}"
        );
        assert_eq!(
            print_expr(&reparsed),
            printed,
            "seed {SEED:#x} case {case}: printing is not idempotent"
        );
        shapes.insert(form(&tree.kind));
    }
    // The generator is not vacuous: every top-level expression form was drawn.
    assert_eq!(shapes.len(), 24, "forms drawn: {shapes:?}");
}

#[test]
fn property_print_parse_round_trips_generated_types() {
    const SEED: u64 = 0x00C0_FFEE_15A0_0002;
    let mut rng = Rng(SEED);
    for case in 0..2000 {
        let t = gen_type(&mut rng, 4);
        let printed = print_type(&t);
        let reparsed = parse_type(&printed).unwrap_or_else(|err| {
            panic!("seed {SEED:#x} case {case}: type print does not re-parse: {err}\n{printed}")
        });
        assert_eq!(
            dump_type(&reparsed),
            dump_type(&t),
            "seed {SEED:#x} case {case}"
        );
    }
}

/// The 1-based line and character column of byte `offset` in `src`.
fn locate(src: &str, offset: usize) -> (u32, u32) {
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (
        u32::try_from(line).expect("fits"),
        u32::try_from(col).expect("fits"),
    )
}

const SNIPPETS: [&str; 28] = [
    " ",
    "\n",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    ",",
    ":",
    ";",
    "=",
    "==",
    "=>",
    "->",
    "|",
    "'",
    "forall x in S: ",
    "x",
    "7",
    "\"",
    "@",
    "é",
    "1.5",
    "??h",
    "process ",
    "next(",
    "<",
];

#[test]
fn fuzz_mutated_fixtures_fail_typed_or_round_trip() {
    const SEED: u64 = 0x00C0_FFEE_15A0_0003;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures: Vec<String> = [
        "../../notes/plan/examples/replicated_register.ctm",
        "../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm",
        "tests/fixtures/SyntaxCoverage.ctm",
        "tests/fixtures/TransitiveClosure.ctm",
    ]
    .iter()
    .map(|p| std::fs::read_to_string(root.join(p)).expect("fixture"))
    .collect();

    let mut rng = Rng(SEED);
    let mut accepted = 0usize;
    let mut codes = std::collections::BTreeSet::new();
    for case in 0..3000 {
        let mut chars: Vec<char> = rng.pick(&fixtures).chars().collect();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(chars.len() + 1);
            if rng.below(2) == 0 && at < chars.len() {
                let len = (1 + rng.below(8)).min(chars.len() - at);
                chars.drain(at..at + len);
            } else {
                let snippet: Vec<char> = rng.pick(&SNIPPETS).chars().collect();
                chars.splice(at..at, snippet);
            }
        }
        let src: String = chars.into_iter().collect();
        let first = parse(&src);
        assert_eq!(
            first,
            parse(&src),
            "seed {SEED:#x} case {case}: nondeterministic"
        );
        match first {
            Ok(file) => {
                accepted += 1;
                let printed = print_file(&file);
                let reparsed = parse(&printed).unwrap_or_else(|err| {
                    panic!("seed {SEED:#x} case {case}: print does not re-parse: {err}\n{printed}")
                });
                assert_eq!(
                    dump_file(&reparsed),
                    dump_file(&file),
                    "seed {SEED:#x} case {case}"
                );
            }
            Err(err) => {
                let (start, end) = (err.span.start as usize, err.span.end as usize);
                assert!(
                    start <= end && end <= src.len(),
                    "seed {SEED:#x} case {case}: {err}"
                );
                assert!(
                    src.is_char_boundary(start) && src.is_char_boundary(end),
                    "seed {SEED:#x} case {case}: span splits a character"
                );
                assert_eq!(
                    locate(&src, start),
                    (err.span.line, err.span.col),
                    "seed {SEED:#x} case {case}: line/col disagree with the offset ({err})"
                );
                codes.insert(err.code());
            }
        }
    }
    // The campaign is not vacuous: some mutants survive and several error kinds occur.
    assert!(accepted > 50, "only {accepted} mutants parsed");
    assert!(codes.len() >= 6, "only {codes:?} error kinds seen");
    assert!(codes.iter().any(|c| c.starts_with("cml.unsupported.")));
}
