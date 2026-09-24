//! A seeded, structure-aware mutator over CML source.
//!
//! Every input is a *host* — a committed corpus source — with one to three mutations
//! applied. The mutations know the lexical shape of CML (identifiers, integer literals,
//! operators, lines, declaration keywords) so most inputs get past the lexer and reach
//! the parser, the elaborator, and the lowering, instead of dying on the first
//! character. The draw is SplitMix64 from a committed seed: no clock, no entropy, no
//! hash order, so a finding reproduces from its seed alone (INV-005).
//!
//! Nothing here grows an input by doubling: every mutation adds at most one line of the
//! host or one bounded snippet (the largest, a twelve-level staircase, is a few
//! kilobytes), so an input stays far below the 32 KiB input budget.

use continuum_cml_syntax::MAX_NESTING;

/// SplitMix64: fully specified, so a seed is the same draw on every platform.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 bits.
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw in `0..n`; `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// One element of `items`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Tokens, keywords, and fragments that are inserted at a random character position.
pub const SNIPPETS: &[&str] = &[
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
    "!=",
    "=>",
    "->",
    "<=>",
    "~>",
    "|",
    "'",
    "..",
    "\\",
    "\"",
    "@",
    "é",
    "\u{0}",
    "\t",
    "//",
    "0",
    "-1",
    "x",
    "state",
    "init",
    "true",
    "false",
    "forall i in 0..3: ",
    "exists j: ",
    "if ",
    " then ",
    " else ",
    "always(",
    "eventually(",
    "step(",
    "stutter(",
    "require ",
    "unchanged ",
    "next ",
    "let v = ",
    "Set[",
    "Map[",
    "Seq[",
    "Nat",
    "Int",
    "Bool",
    "union",
    "intersect",
    "subseteq",
    "notin",
    " % ",
    " / ",
    "min(",
    "max(",
    "len(",
    ".size()",
    "[0 := 1]",
    "{ a: 1 }",
    "(1, 2)",
    "1.5",
    "??h",
    "process ",
    "next(",
    "choose ",
    "<",
    ">",
];

/// Whole declarations inserted at a line boundary: ordinary ones that interact with the
/// host's names, and every out-of-fragment form the parser and elaborator name.
pub const DECLS: &[&str] = &[
    "type T0 = Nat",
    "type Loop = Loop",
    "type A1 = B1\ntype B1 = A1",
    "enum Color { Red, Green, Red }",
    "const K: Nat",
    "def f(n: Nat): Nat = if n == 0 then 0 else f(n - 1) + 1",
    "def g(n: Nat): Nat = g(n)",
    "def h(n: Nat): Nat = k(n)\ndef k(n: Nat): Nat = h(n)",
    "invariant Big { f(64) >= 0 }",
    "invariant Deep { f(65) >= 0 }",
    "invariant Wide { forall i in 0..100000: i >= 0 }",
    "invariant Huge { 9223372036854775807 + 1 > 0 }",
    "invariant BeyondI64 { 9223372036854775808 > 0 }",
    "action Nop { }",
    "action Param(p: Bool) { require p }",
    "init { true }",
    "behavior B = always(true)",
    "fairness weak Nop",
    "process P(p in Nodes) {\n}",
    "view V from P {\n}",
    "symmetry rotate(Node)",
    "check deadlock",
    "import Paxos",
    "extern type Clock",
    "progress P {\n  true\n}",
    "eventually E {\n  true\n}",
    "transition invariant T {\n  true\n}",
    "hyperproperty H {\n  true\n}",
    "invariant Hole { ??guard(x) }",
    "invariant Nx { next(x) == x }",
    "invariant Fl { 1 < 1.5 }",
];

/// One nesting level each: the text before and after the nested expression.
const NESTERS: &[(&str, &str)] = &[
    ("(", ")"),
    ("!", ""),
    ("-", ""),
    ("(", " + 1)"),
    ("if true then ", " else 0"),
    ("min(0, ", ")"),
    ("[", "][0]"),
    ("{ a: ", " }.a"),
    ("{", "}"),
    ("forall q in 0..1: ", ""),
    ("always(", ")"),
    ("(", ")'"),
    ("{ 0 -> ", " }[0]"),
    ("[0][0 := ", "]"),
];

/// One type nesting level each: the text before and after the nested type.
const TYPE_NESTERS: &[(&str, &str)] = &[
    ("Set[", "] -> Nat"),
    ("", " -> Nat"),
    ("Nat -> ", ""),
    ("Set[", "]"),
    ("Map[Nat, ", "]"),
    ("(", ", Nat)"),
    ("{ a: ", " }"),
    ("(", ")"),
];

/// The postfix operators, for chains of them.
const POSTFIX: &[&str] = &["'", ".f", ".m()", "[0]", "[0 := 1]"];

/// Integer literals at the edges of the lexer's and the elaborator's ranges.
const EXTREMES: &[&str] = &[
    "0",
    "1",
    "64",
    "65",
    "4096",
    "2147483648",
    "9223372036854775807",
    "9223372036854775808",
    "18446744073709551615",
    "18446744073709551616",
];

/// Operators that a mutation swaps for one another.
const OPERATORS: &[&str] = &[
    "==", "!=", "<=", ">=", "<", ">", "&&", "||", "=>", "+", "-", "*", "in", "notin", "union", "..",
];

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Character ranges, `start..end`.
type Ranges = Vec<(usize, usize)>;

/// The character ranges of identifier-shaped runs and of digit runs in `chars`.
fn runs(chars: &[char]) -> (Ranges, Ranges) {
    let (mut idents, mut ints) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_ident_start(c) {
            let start = i;
            while i < chars.len() && is_ident_char(chars[i]) {
                i += 1;
            }
            idents.push((start, i));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            ints.push((start, i));
        } else {
            i += 1;
        }
    }
    (idents, ints)
}

/// The character offsets at which a line starts.
fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        chars
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .filter(|i| *i < chars.len()),
    );
    starts
}

/// The line containing character `at`, without its newline.
fn line_at(chars: &[char], starts: &[usize], k: usize) -> (usize, usize) {
    let start = starts[k];
    let end = chars[start..]
        .iter()
        .position(|c| *c == '\n')
        .map_or(chars.len(), |p| start + p);
    (start, end)
}

/// Apply one mutation to `chars`. `other` is a second host, for crossover.
fn mutate(rng: &mut Rng, chars: &mut Vec<char>, other: &[char]) {
    let (idents, ints) = runs(chars);
    let starts = line_starts(chars);
    // Weighted toward the mutations that keep the source lexically whole (a literal, a
    // name, an operator, a declaration, a line), so most inputs get past the parser.
    const WEIGHTS: [usize; 14] = [1, 1, 2, 1, 3, 4, 3, 1, 3, 1, 1, 1, 1, 1];
    let mut draw = rng.below(WEIGHTS.iter().sum());
    let mut op = 0;
    while draw >= WEIGHTS[op] {
        draw -= WEIGHTS[op];
        op += 1;
    }
    match op {
        // Delete a short range.
        0 if !chars.is_empty() => {
            let at = rng.below(chars.len());
            let len = (1 + rng.below(12)).min(chars.len() - at);
            chars.drain(at..at + len);
        }
        // Insert a token or fragment anywhere.
        1 => {
            let at = rng.below(chars.len() + 1);
            chars.splice(at..at, rng.pick(SNIPPETS).chars());
        }
        // Duplicate one line.
        2 => {
            let k = rng.below(starts.len());
            let (s, e) = line_at(chars, &starts, k);
            let mut copy: Vec<char> = chars[s..e].to_vec();
            copy.push('\n');
            chars.splice(s..s, copy);
        }
        // Swap two lines.
        3 if starts.len() > 1 => {
            let (a, b) = (rng.below(starts.len()), rng.below(starts.len()));
            let (a, b) = (a.min(b), a.max(b));
            if a != b {
                let (sa, ea) = line_at(chars, &starts, a);
                let (sb, eb) = line_at(chars, &starts, b);
                let la: Vec<char> = chars[sa..ea].to_vec();
                let lb: Vec<char> = chars[sb..eb].to_vec();
                chars.splice(sb..eb, la);
                chars.splice(sa..ea, lb);
            }
        }
        // Replace an integer literal with an extreme one.
        4 if !ints.is_empty() => {
            let (s, e) = *rng.pick(&ints);
            chars.splice(s..e, rng.pick(EXTREMES).chars());
        }
        // Replace one identifier occurrence with another identifier of the host.
        5 if idents.len() > 1 => {
            let (s, e) = *rng.pick(&idents);
            let (fs, fe) = *rng.pick(&idents);
            let from: Vec<char> = chars[fs..fe].to_vec();
            chars.splice(s..e, from);
        }
        // Swap an operator for another.
        6 => {
            let text: String = chars.iter().collect();
            let op = rng.pick(OPERATORS);
            let found: Vec<usize> = text.match_indices(op).map(|(i, _)| i).collect();
            if !found.is_empty() {
                let byte = *rng.pick(&found);
                let at = text[..byte].chars().count();
                let len = op.chars().count();
                chars.splice(at..at + len, rng.pick(OPERATORS).chars());
            }
        }
        // Insert a line of another host at a line boundary (crossover).
        7 if !other.is_empty() => {
            let ostarts = line_starts(other);
            let k = rng.below(ostarts.len());
            let (s, e) = line_at(other, &ostarts, k);
            let mut line: Vec<char> = other[s..e].to_vec();
            line.push('\n');
            let at = *rng.pick(&starts);
            chars.splice(at..at, line);
        }
        // Insert a whole declaration at a line boundary.
        8 => {
            let mut decl: Vec<char> = rng.pick(DECLS).chars().collect();
            decl.push('\n');
            let at = *rng.pick(&starts);
            chars.splice(at..at, decl);
        }
        // Wrap an identifier in up to just past the parser's nesting bound of levels, each
        // level a form drawn from `NESTERS`, so mixed deep trees reach every recursive
        // pass (parse, dump, print, elaborate, identity, lower) at their depth bound.
        9 if !idents.is_empty() => {
            let (s, e) = *rng.pick(&idents);
            let depth = 1 + rng.below(MAX_NESTING as usize + 3);
            let (mut open, mut close) = (Vec::new(), Vec::new());
            for _ in 0..depth {
                let (pre, post) = *rng.pick(NESTERS);
                open.extend(pre.chars());
                close.splice(0..0, post.chars());
            }
            chars.splice(e..e, close);
            chars.splice(s..s, open);
        }
        // Prime an identifier occurrence.
        10 if !idents.is_empty() => {
            let (_, e) = *rng.pick(&idents);
            chars.insert(e, '\'');
        }
        // Follow an identifier with a postfix chain up to just past the nesting bound.
        11 if !idents.is_empty() => {
            let (_, e) = *rng.pick(&idents);
            let n = 1 + rng.below(MAX_NESTING as usize + 3);
            let mut chain = Vec::new();
            for _ in 0..n {
                chain.extend(rng.pick(POSTFIX).chars());
            }
            chars.splice(e..e, chain);
        }
        // Replace an identifier with a staircase: each level a parenthesized chain as
        // the left operand of a longer one, the shape whose tree is quadratic in the
        // levels when a chain's operands are not counted where they end up.
        12 if !idents.is_empty() => {
            let (s, e) = *rng.pick(&idents);
            let levels = 1 + rng.below(12);
            let op = *rng.pick(&[" + 1", " && true", " union {0}", " - 1"]);
            let mut stair: String = chars[s..e].iter().collect();
            for level in 0..levels {
                let width = (level + 1) * (1 + rng.below(8));
                stair = format!("({stair}){}", op.repeat(width));
            }
            chars.splice(s..e, stair.chars());
        }
        // Declare a constant of a nested type near the nesting bound: each level wraps
        // the type so far in a form drawn from `TYPE_NESTERS`, including the function
        // arrow, whose left side is re-attached below a new node (cr-3rqxh8).
        13 => {
            let levels = 1 + rng.below(MAX_NESTING as usize + 3);
            let mut ty = String::from("Nat");
            for _ in 0..levels {
                let (pre, post) = *rng.pick(TYPE_NESTERS);
                ty = format!("{pre}{ty}{post}");
            }
            let mut decl: Vec<char> = format!("const FuzzT: {ty}\n").chars().collect();
            let at = *rng.pick(&starts);
            chars.splice(at..at, decl.drain(..));
        }
        _ => {
            let at = rng.below(chars.len() + 1);
            chars.splice(at..at, rng.pick(SNIPPETS).chars());
        }
    }
}

/// One mutated input: a host with one mutation (half the draws), two (a third), or
/// three (a sixth).
#[must_use]
pub fn generate(rng: &mut Rng, hosts: &[String]) -> String {
    let host = rng.pick(hosts);
    let other: Vec<char> = rng.pick(hosts).chars().collect();
    let mut chars: Vec<char> = host.chars().collect();
    let count = match rng.below(6) {
        0..=2 => 1,
        3 | 4 => 2,
        _ => 3,
    };
    for _ in 0..count {
        mutate(rng, &mut chars, &other);
    }
    chars.into_iter().collect()
}
