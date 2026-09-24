//! Typed, source-located errors of the CML Finite core parser (PR 15a, bn-1sf).
//!
//! | Claim | Test |
//! |---|---|
//! | every out-of-fragment construct is a typed `Unsupported` error at the construct | [`negative_every_unsupported_construct_is_typed_and_located`] |
//! | the unsupported table covers every `Unsupported` variant, with distinct codes | [`negative_the_unsupported_table_is_exhaustive`] |
//! | unsupported words stay ordinary names outside declaration or statement position | [`positive_unsupported_words_are_ordinary_names_in_expressions`] |
//! | malformed input is a typed error at the defect | [`negative_malformed_input_is_typed_and_located`] |
//! | the first defect in source order wins, lexical or syntactic | [`negative_the_first_defect_in_source_order_is_reported`] |
//! | columns count characters, not bytes | [`negative_columns_count_characters`] |
//! | nesting and size bounds refuse instead of overflowing | [`adversarial_nesting_and_size_bounds_refuse`] |
//! | the deepest accepted tree fits in half a default thread stack, unoptimized | [`adversarial_maximum_depth_fits_in_half_a_default_stack`] |

use continuum_cml_syntax::{
    MAX_NESTING, MAX_SOURCE_BYTES, ParseError, ParseErrorKind, Unsupported, parse, parse_expr,
};

fn err_of(src: &str) -> ParseError {
    match parse(src) {
        Ok(tree) => panic!("expected an error for {src:?}, parsed {tree:?}"),
        Err(e) => e,
    }
}

fn slice(src: &str, e: &ParseError) -> String {
    src[e.span.start as usize..e.span.end as usize].to_owned()
}

/// `(construct, source, text the error span must cover, line, col)`.
const UNSUPPORTED: &[(Unsupported, &str, &str, u32, u32)] = &[
    (
        Unsupported::ParameterizedModel,
        "model M<const N: Nat> {\n}\n",
        "<",
        1,
        8,
    ),
    (
        Unsupported::ParameterizedSort,
        "module M\nconst Xs: Set[Node<3>]\n",
        "<",
        2,
        19,
    ),
    (
        Unsupported::Process,
        "module M\nprocess P(p in Nodes) {\n}\n",
        "process",
        2,
        1,
    ),
    (
        Unsupported::Await,
        "module M\naction A {\n  await ready\n}\n",
        "await",
        3,
        3,
    ),
    (
        Unsupported::Goto,
        "module M\naction A {\n  goto Done\n}\n",
        "goto",
        3,
        3,
    ),
    (
        Unsupported::View,
        "module M\nview Service from Protocol {\n}\n",
        "view",
        2,
        1,
    ),
    (
        Unsupported::ForgeBlock,
        "model M {\n  forge {\n  }\n}\n",
        "forge",
        2,
        3,
    ),
    (
        Unsupported::SynthesisHole,
        "module M\ninvariant I { ??guard(x) }\n",
        "??guard",
        2,
        15,
    ),
    (
        Unsupported::ProgressProperty,
        "module M\nprogress P {\n  true\n}\n",
        "progress",
        2,
        1,
    ),
    (
        Unsupported::EventuallyProperty,
        "module M\neventually RequestCompletes {\n  true\n}\n",
        "eventually",
        2,
        1,
    ),
    (
        Unsupported::TransitionInvariant,
        "module M\ntransition invariant DurableAck {\n  true\n}\n",
        "transition",
        2,
        1,
    ),
    (
        Unsupported::Hyperproperty,
        "module M\nhyperproperty OD {\n  true\n}\n",
        "hyperproperty",
        2,
        1,
    ),
    (
        Unsupported::Symmetry,
        "module M\nsymmetry rotate(Node)\n",
        "symmetry",
        2,
        1,
    ),
    (
        Unsupported::CheckDirective,
        "module M\ncheck deadlock\n",
        "check",
        2,
        1,
    ),
    (
        Unsupported::ModuleImport,
        "module M\nimport Paxos\n",
        "import",
        2,
        1,
    ),
    (
        Unsupported::OpaqueDomain,
        "module M\nextern type Clock\n",
        "extern",
        2,
        1,
    ),
    (
        Unsupported::Choose,
        "module M\naction A {\n  let n = choose Node where alive[n]\n}\n",
        "choose",
        3,
        11,
    ),
    (
        Unsupported::TemporalNext,
        "module M\ninvariant I { next(x) == x }\n",
        "next",
        2,
        15,
    ),
    (
        Unsupported::FloatLiteral,
        "module M\ninvariant I { x < 1.5 }\n",
        "1.5",
        2,
        19,
    ),
];

#[test]
fn negative_every_unsupported_construct_is_typed_and_located() {
    for &(what, src, text, line, col) in UNSUPPORTED {
        let e = err_of(src);
        assert_eq!(
            e.kind,
            ParseErrorKind::Unsupported(what),
            "{src:?}: wrong kind: {e}"
        );
        assert!(e.is_unsupported());
        assert_eq!(e.code(), what.code());
        assert_eq!(slice(src, &e), text, "{src:?}: span covers the wrong text");
        assert_eq!(
            (e.span.line, e.span.col),
            (line, col),
            "{src:?}: wrong location"
        );
        let shown = e.to_string();
        assert!(
            shown.starts_with(&format!("{line}:{col}: {}: ", what.code())),
            "{shown}"
        );
        assert!(
            shown.contains("outside the CML Finite core fragment"),
            "{shown}"
        );
    }
}

#[test]
fn negative_the_unsupported_table_is_exhaustive() {
    for what in Unsupported::ALL {
        assert!(
            UNSUPPORTED.iter().any(|row| row.0 == what),
            "{what:?} has no negative fixture"
        );
    }
    let mut codes: Vec<&str> = Unsupported::ALL.iter().map(|u| u.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(
        codes.len(),
        Unsupported::ALL.len(),
        "codes must be distinct"
    );
}

#[test]
fn negative_other_spellings_of_unsupported_constructs_are_typed() {
    let cases: &[(&str, Unsupported)] = &[
        ("module M\nrefines Protocol\n", Unsupported::View),
        ("module M\nuse Paxos\n", Unsupported::ModuleImport),
        ("module M\nopaque type Clock\n", Unsupported::OpaqueDomain),
        (
            "module M\naction A {\n  next(x) == x\n}\n",
            Unsupported::TemporalNext,
        ),
        (
            "module M\ninit {\n  next(x) == x\n}\n",
            Unsupported::TemporalNext,
        ),
        (
            "module M\naction A(p: Map[Node<N>, Nat]) {\n}\n",
            Unsupported::ParameterizedSort,
        ),
    ];
    for (src, what) in cases {
        assert_eq!(
            err_of(src).kind,
            ParseErrorKind::Unsupported(*what),
            "{src:?}"
        );
    }
}

#[test]
fn positive_unsupported_words_are_ordinary_names_in_expressions() {
    let src = "module M\nstate {\n  check: Bool\n  view: Bool\n}\ninvariant I {\n  check == view\n  process(goto, await)\n}\n";
    parse(src).expect("contextual words are names outside declaration position");
}

/// `(code, source, text the error span must cover, line, col)`.
const MALFORMED: &[(&str, &str, &str, u32, u32)] = &[
    (
        "cml.lex.unexpected_character",
        "module M\ninvariant I { x @ y }\n",
        "@",
        2,
        17,
    ),
    (
        "cml.lex.unexpected_character",
        "module M\ninvariant I { x & y }\n",
        "&",
        2,
        17,
    ),
    (
        "cml.lex.unexpected_character",
        "module M\ninvariant I { x ? y }\n",
        "?",
        2,
        17,
    ),
    (
        "cml.lex.unterminated_string",
        "module M\ninvariant I { \"abc }\n",
        "\"abc }",
        2,
        15,
    ),
    (
        "cml.lex.invalid_escape",
        "module M\ninvariant I { \"a\\qb\" }\n",
        "\\q",
        2,
        17,
    ),
    (
        "cml.lex.integer_too_large",
        "module M\ninvariant I { x < 18446744073709551616 }\n",
        "18446744073709551616",
        2,
        19,
    ),
    ("cml.parse.unexpected_token", "type X\n", "type", 1, 1),
    (
        "cml.parse.unexpected_token",
        "module M\naction A(n Node) {\n}\n",
        "Node",
        2,
        12,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\ninvariant I { x == }\n",
        "}",
        2,
        20,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\nstate {\n  x: Nat y: Nat\n}\n",
        "y",
        3,
        10,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\nfairness weak A B\n",
        "B",
        2,
        17,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\nfairness fair A\n",
        "fair",
        2,
        10,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\ninvariant I {\n  x == 1\n",
        "",
        4,
        1,
    ),
    (
        "cml.parse.unexpected_token",
        "model M {\n  type T\n}\ntype U\n",
        "type",
        4,
        1,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\ninvariant I { x y }\n",
        "y",
        2,
        17,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\ninvariant I { () }\n",
        ")",
        2,
        16,
    ),
    (
        "cml.parse.unexpected_token",
        "module M\nwidget W\n",
        "widget",
        2,
        1,
    ),
    (
        "cml.parse.chained_operator",
        "module M\ninvariant I { a == b == c }\n",
        "==",
        2,
        22,
    ),
    (
        "cml.parse.chained_operator",
        "module M\ninvariant I { a ~> b ~> c }\n",
        "~>",
        2,
        22,
    ),
    (
        "cml.parse.statement_not_allowed",
        "module M\ninvariant I {\n  require x\n}\n",
        "require",
        3,
        3,
    ),
    (
        "cml.parse.statement_not_allowed",
        "module M\ninit {\n  next x = 1\n}\n",
        "next",
        3,
        3,
    ),
    (
        "cml.parse.statement_not_allowed",
        "module M\ninit {\n  unchanged x\n}\n",
        "unchanged",
        3,
        3,
    ),
];

#[test]
fn negative_malformed_input_is_typed_and_located() {
    for &(code, src, text, line, col) in MALFORMED {
        let e = err_of(src);
        assert_eq!(e.code(), code, "{src:?}: {e}");
        assert!(!e.is_unsupported(), "{src:?}: malformed is not unsupported");
        assert_eq!(
            slice(src, &e),
            text,
            "{src:?}: span covers the wrong text ({e})"
        );
        assert_eq!((e.span.line, e.span.col), (line, col), "{src:?}: {e}");
    }
}

#[test]
fn negative_the_first_defect_in_source_order_is_reported() {
    // A syntax error before a lexical error: the syntax error is reported.
    let src = "module M\ninvariant I { x == }\ninvariant J { 1.5 @ }\n";
    let e = err_of(src);
    assert_eq!(e.code(), "cml.parse.unexpected_token");
    assert_eq!(e.span.line, 2);

    // A lexical error before a syntax error: the lexical error is reported.
    let src = "module M\ninvariant I { x @ }\ninvariant J { x == }\n";
    let e = err_of(src);
    assert_eq!(e.code(), "cml.lex.unexpected_character");
    assert_eq!(e.span.line, 2);

    // An unsupported construct before a syntax error: the construct is reported.
    let src = "module M\nsymmetry rotate(Node)\ninvariant J { x == }\n";
    assert_eq!(
        err_of(src).kind,
        ParseErrorKind::Unsupported(Unsupported::Symmetry)
    );
}

#[test]
fn negative_columns_count_characters() {
    // `é` is two bytes and one column.
    let src = "module M // é\ninvariant I { \"é\" @ }\n";
    let e = err_of(src);
    assert_eq!(e.code(), "cml.lex.unexpected_character");
    assert_eq!((e.span.line, e.span.col), (2, 19));
    assert_eq!(slice(src, &e), "@");
}

/// A postfix chain deepens the tree by one per operator, so it counts against the nesting
/// bound, like an infix chain (bn-1nmq). Before, `x` followed by thousands of primes, or
/// of `.f`, `.m()`, `[k]`, or `[k := v]`, parsed to a tree that deep, and the recursive
/// passes over it (dump, print, elaboration, `Drop`) overflowed the stack. It was found
/// with adversarial inputs written for the CML fuzz harness
/// (`continuum-cml-elab/tests/cml_fuzz.rs`), whose corpus keeps the three
/// `oversized-postfix-*` cases and whose mutator now draws postfix chains too.
#[test]
fn adversarial_postfix_chains_count_against_the_nesting_bound() {
    let bound = usize::try_from(MAX_NESTING).expect("small");
    let forms: [(&str, &str); 5] = [
        ("prime", "'"),
        ("field", ".f"),
        ("method", ".m()"),
        ("index", "[0]"),
        ("update", "[0 := 1]"),
    ];
    for (name, step) in forms {
        let within = format!("x{}", step.repeat(bound - 2));
        parse_expr(&within).unwrap_or_else(|e| panic!("{name}: a chain within the bound: {e}"));
        for n in [bound + 1, 5000] {
            let src = format!("x{}", step.repeat(n));
            let e = parse_expr(&src).expect_err("an over-long postfix chain is refused");
            assert_eq!(e.kind, ParseErrorKind::NestingTooDeep, "{name} × {n}");
            // Refused at the operator that passes the bound, not at the end of the chain.
            let at = e.span.start as usize;
            assert!(src[at..].starts_with(&step[..1]), "{name}: refused at {at}");
            assert!(
                at < 1 + step.len() * (bound + 1),
                "{name}: refused late, at {at}"
            );
        }
    }
    // A chain inside groups shares one bound with them. The bound is conservative here:
    // the tree is only 31 deep (groups are not nodes), but the chain is checked against
    // a level that counts the forty enclosing groups, so it is refused. Pinned so that a
    // change to this trade-off is a deliberate, visible one.
    let mixed = format!("{}x{}{}", "(".repeat(40), "'".repeat(30), ")".repeat(40));
    assert_eq!(
        parse_expr(&mixed)
            .expect_err("a chain inside forty groups passes the counted level")
            .kind,
        ParseErrorKind::NestingTooDeep
    );
    // In a whole model, the refusal is the first defect.
    let model = format!(
        "module M\nstate {{ x: Nat }}\naction A {{\n  x{} == x\n}}\n",
        "'".repeat(5000)
    );
    assert_eq!(err_of(&model).kind, ParseErrorKind::NestingTooDeep);
}

/// The depth of the tree an expression parses to, without recursion.
fn tree_depth(root: &continuum_cml_syntax::ast::Expr) -> usize {
    use continuum_cml_syntax::ast::{Binder, Expr, ExprKind as K};
    fn domains(bs: &[Binder]) -> impl Iterator<Item = &Expr> {
        bs.iter().filter_map(|b| b.domain.as_ref())
    }
    let mut deepest = 0;
    let mut stack = vec![(root, 1_usize)];
    while let Some((e, d)) = stack.pop() {
        deepest = deepest.max(d);
        let children: Vec<&Expr> = match &e.kind {
            K::Int(_) | K::Bool(_) | K::Str(_) | K::Name(_) | K::WholeState | K::EmptyBraces => {
                vec![]
            }
            K::Prime(a) | K::Unary(_, a) | K::Temporal(_, a) | K::Field(a, _) => vec![a],
            K::Binary(_, a, b) | K::Index(a, b) => vec![a, b],
            K::If(a, b, c) | K::Update(a, b, c) => vec![a, b, c],
            K::Quant(_, bs, body) => domains(bs).chain([&**body]).collect(),
            K::Call(_, xs) | K::Tuple(xs) | K::SetLit(xs) | K::SeqLit(xs) => xs.iter().collect(),
            K::Method(a, _, xs) => std::iter::once(&**a).chain(xs).collect(),
            K::MapLit(kvs) => kvs.iter().flat_map(|(k, v)| [k, v]).collect(),
            K::Record(fs) => fs.iter().map(|(_, v)| v).collect(),
            K::SetComp(a, bs, w) => std::iter::once(&**a)
                .chain(domains(bs))
                .chain(w.as_deref())
                .collect(),
            K::MapComp(a, b, bs, w) => [&**a, &**b]
                .into_iter()
                .chain(domains(bs))
                .chain(w.as_deref())
                .collect(),
        };
        stack.extend(children.into_iter().map(|c| (c, d + 1)));
    }
    deepest
}

/// The bound is on the depth of the *tree*, and an operator chain puts every operand
/// parsed before it one level deeper (bn-1nmq). Before, the parser bounded the level each
/// operand was entered at, so a staircase — each level a parenthesized chain as the left
/// operand of a longer chain — was admitted with a tree about `MAX_NESTING² / 2` levels
/// deep: 7.5 KB of source, 1,830 levels, a stack overflow in `dump_file` on a default
/// 2 MiB thread. Every accepted expression here is measured without recursion.
#[test]
fn adversarial_chains_cannot_stack_past_the_bound() {
    let bound = usize::try_from(MAX_NESTING).expect("small");
    // Staircases with each operator family, and with postfix chains, the left-deep forms.
    let steps: [(&str, &str, &str); 4] = [
        ("(", ")", " + x"),
        ("(", ")", " && x"),
        ("(", ")", " union x"),
        ("(", ")", "'"),
    ];
    for (open, close, step) in steps {
        for width in [2, 8, 60] {
            let mut e = "x".to_owned();
            for level in (0..60).rev() {
                e = format!("{open}{e}{close}{}", step.repeat((60 - level).min(width)));
            }
            match parse_expr(&e) {
                Ok(tree) => {
                    let depth = tree_depth(&tree);
                    assert!(
                        depth <= bound,
                        "{step:?} × {width}: a tree {depth} deep parsed"
                    );
                }
                Err(err) => assert_eq!(err.kind, ParseErrorKind::NestingTooDeep, "{step:?}"),
            }
        }
    }
    // Every early right operand moves down too: `x + (deep) + x + …`.
    let deep = format!("{}x{}", "(".repeat(40), ")".repeat(40));
    let src = format!("x + {deep}{}", " + x".repeat(40));
    match parse_expr(&src) {
        Ok(tree) => assert!(tree_depth(&tree) <= bound),
        Err(err) => assert_eq!(err.kind, ParseErrorKind::NestingTooDeep),
    }
    // A method argument moves down with the chain after it.
    let src = format!("x.m({}){}", vec!["x"; 50].join(" + "), "'".repeat(30));
    match parse_expr(&src) {
        Ok(tree) => assert!(tree_depth(&tree) <= bound),
        Err(err) => assert_eq!(err.kind, ParseErrorKind::NestingTooDeep),
    }
    // Exactness for a flat chain: `n` operands are a tree `n` deep, accepted exactly up to
    // the bound. (Inside parentheses the bound is conservative: a chain there is checked
    // against a level that counts every enclosing group; see the `mixed` case above.)
    for n in 1..=bound + 1 {
        let src = vec!["x"; n].join(" + ");
        match parse_expr(&src) {
            Ok(tree) => {
                assert!(
                    n <= bound,
                    "a chain of {n} operands, past the bound, parsed"
                );
                assert_eq!(tree_depth(&tree), n);
            }
            Err(err) => {
                assert_eq!(err.kind, ParseErrorKind::NestingTooDeep);
                assert!(
                    n > bound,
                    "a chain of {n} operands, within the bound, was refused"
                );
            }
        }
    }
}

/// The depth of a type tree, without recursion.
fn type_tree_depth(root: &continuum_cml_syntax::ast::TypeExpr) -> usize {
    use continuum_cml_syntax::ast::TypeKind as T;
    let mut deepest = 0;
    let mut stack = vec![(root, 1_usize)];
    while let Some((t, d)) = stack.pop() {
        deepest = deepest.max(d);
        match &t.kind {
            T::Named(_) => {}
            T::Applied(_, xs) | T::Tuple(xs) => stack.extend(xs.iter().map(|x| (x, d + 1))),
            T::Function(a, b) => stack.extend([(&**a, d + 1), (&**b, d + 1)]),
            T::Record(fs) => stack.extend(fs.iter().map(|(_, x)| (x, d + 1))),
        }
    }
    deepest
}

/// A function type's left side is re-attached below the new `Function` node, so it is
/// charged one level more when the arrow arrives (bn-1nmq, cr-3rqxh8). Before, `Tn =
/// Set[Tn-1] -> Nat` written out was counted at `n + 1` levels though its tree is
/// `2n + 1` deep, and `T63` (127 deep) parsed.
#[test]
fn adversarial_function_types_cannot_stack_past_the_bound() {
    let bound = usize::try_from(MAX_NESTING).expect("small");
    let t = |n: usize| {
        let mut ty = String::from("Nat");
        for _ in 0..n {
            ty = format!("Set[{ty}] -> Nat");
        }
        ty
    };
    for n in 0..=64 {
        match continuum_cml_syntax::parse_type(&t(n)) {
            Ok(ty) => {
                assert!(2 * n < bound, "T{n} accepted, a tree {} deep", 2 * n + 1);
                assert_eq!(type_tree_depth(&ty), 2 * n + 1);
            }
            Err(e) => {
                assert!(2 * n >= bound, "T{n} refused within the bound: {e}");
                assert_eq!(e.kind, ParseErrorKind::NestingTooDeep);
                // Refused at an arrow, the first one whose left side passes the bound,
                // until the `Set[` nesting alone passes it (from T64 on), where the
                // innermost type is refused first, in source order.
                let src = t(n);
                if n < bound {
                    assert!(src[e.span.start as usize..].starts_with("->"), "T{n}: {e}");
                }
            }
        }
    }
    // Left-nested arrows: `((Nat -> Nat) -> Nat) -> …` is left-deep too.
    for n in [bound - 2, bound - 1, bound, 5000] {
        let mut ty = String::from("Nat");
        for _ in 0..n {
            ty = format!("({ty}) -> Nat");
        }
        match continuum_cml_syntax::parse_type(&ty) {
            Ok(tree) => assert!(type_tree_depth(&tree) <= bound, "{n} left arrows"),
            Err(e) => assert_eq!(e.kind, ParseErrorKind::NestingTooDeep, "{n}"),
        }
    }
}

#[test]
fn adversarial_nesting_and_size_bounds_refuse() {
    let depth = usize::try_from(MAX_NESTING).expect("small") - 2;
    let ok = format!("{}x{}", "(".repeat(depth), ")".repeat(depth));
    parse_expr(&ok).expect("nesting within the bound parses");

    for deep in [
        format!("{}x{}", "(".repeat(5000), ")".repeat(5000)),
        format!("{}x", "!".repeat(5000)),
        format!("{}x{}", "[".repeat(5000), "]".repeat(5000)),
        format!("{}x{}", "{".repeat(5000), "}".repeat(5000)),
        format!("{}x", "forall a: ".repeat(5000)),
    ] {
        let e = parse_expr(&deep).expect_err("deep nesting is refused");
        assert_eq!(e.kind, ParseErrorKind::NestingTooDeep);
    }
    let deep_type = format!(
        "module M\nconst C: {}Nat{}\n",
        "Set[".repeat(5000),
        "]".repeat(5000)
    );
    assert_eq!(err_of(&deep_type).kind, ParseErrorKind::NestingTooDeep);

    // An infix chain deepens the tree by one per operator, so it counts too: the bound
    // limits the depth of the returned tree, not only the parser's own recursion.
    let fits = vec!["x"; depth].join(" && ");
    parse_expr(&fits).expect("a chain within the bound parses");
    for op in [" && ", " => ", " + "] {
        let long = vec!["x"; 20_000].join(op);
        let e = parse_expr(&long).expect_err("an over-long chain is refused");
        assert_eq!(e.kind, ParseErrorKind::NestingTooDeep, "{op}");
    }

    let len = usize::try_from(MAX_SOURCE_BYTES).expect("fits") + 1;
    let big = " ".repeat(len);
    assert_eq!(err_of(&big).kind, ParseErrorKind::InputTooLarge);
}

/// One source per nesting form, nested `d` levels deep.
fn nested(form: &str, d: usize) -> String {
    match form {
        "paren" => format!("{}x{}", "(".repeat(d), ")".repeat(d)),
        "not" => format!("{}x", "!".repeat(d)),
        "seq" => format!("{}x{}", "[".repeat(d), "]".repeat(d)),
        "set" => format!("{}x{}", "{".repeat(d), "}".repeat(d)),
        "map-comp" => format!("{}x{}", "{x -> ".repeat(d), " | x}".repeat(d)),
        "set-comp" => format!("{}x{}", "{".repeat(d), " | x in s where p}".repeat(d)),
        "record" => format!("{}x{}", "{a: ".repeat(d), "}".repeat(d)),
        "forall" => format!("{}x", "forall a in b: ".repeat(d)),
        "if" => format!("{}x", "if a then b else ".repeat(d)),
        "call" => format!("{}x{}", "f(".repeat(d), ")".repeat(d)),
        "method" => format!("{}x{}", "y.m(".repeat(d), ")".repeat(d)),
        "update" => format!("x{}", "[y := z]".repeat(d)),
        "and" => vec!["x"; d + 1].join(" && "),
        "implies" => vec!["x"; d + 1].join(" => "),
        "always" => format!("{}x{}", "always(".repeat(d), ")".repeat(d)),
        _ => unreachable!("unknown form {form}"),
    }
}

const FORMS: [&str; 15] = [
    "paren", "not", "seq", "set", "map-comp", "set-comp", "record", "forall", "if", "call",
    "method", "update", "and", "implies", "always",
];

#[test]
fn adversarial_maximum_depth_fits_in_half_a_default_stack() {
    // Find, per form, the deepest nesting the parser accepts, then parse, dump, print,
    // and drop it on a thread with half of Rust's default 2 MiB stack.
    let worker = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for form in FORMS {
                let mut d = usize::try_from(MAX_NESTING).expect("small") + 1;
                let tree = loop {
                    match parse_expr(&nested(form, d)) {
                        Ok(tree) => break tree,
                        Err(e) => {
                            assert_eq!(
                                e.kind,
                                ParseErrorKind::NestingTooDeep,
                                "{form} at {d}: {e}"
                            );
                            d -= 1;
                        }
                    }
                };
                assert!(
                    d + 4 >= usize::try_from(MAX_NESTING).expect("small"),
                    "{form}: only {d}"
                );
                let printed = continuum_cml_syntax::print::print_expr(&tree);
                let dumped = continuum_cml_syntax::dump::dump_expr(&tree);
                let again = parse_expr(&printed).expect("the print re-parses");
                assert_eq!(
                    continuum_cml_syntax::dump::dump_expr(&again),
                    dumped,
                    "{form}"
                );
                drop(tree);
                drop(again);
            }
        })
        .expect("spawn");
    worker
        .join()
        .expect("no stack overflow and no failed assertion");
}
