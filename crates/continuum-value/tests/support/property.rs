//! The Phase A property harness: a seeded generator, a deterministic shrinker, and a
//! textual counterexample form that replays without the generator (`bn-221j`).
//!
//! # Why this exists, and why it is not `proptest`
//!
//! The workspace declares **exactly one** external dependency — `blake3`, entered in
//! `bn-30eym` to fill the ADR-0013 hash seam — and
//! `tools/governance/dependency-rationale.toml` states the bar for a second one: "an honest
//! class, a stated cost, and a tree small enough to read". `proptest` is a nine-plus-crate
//! tree including a full RNG stack, and it would be a `dev-dependencies` edge in four
//! member manifests. It is also the wrong shape for this project's determinism obligations:
//!
//! > Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation
//! > through explicit capabilities.
//! >
//! > — `INV-005`, `notes/plan/plan.md:323`
//!
//! A property harness whose default is "seed from the OS unless told otherwise" is an
//! ambient-entropy capability wearing a test's clothes. Here entropy is a `u64` written
//! into a [`Plan`] in the test source, and there is no other source: [`Prng`] has one
//! constructor and it takes the seed.
//!
//! The idiom this file generalizes is already in the workspace.
//! `crates/continuum-workspace/tests/deterministic_ordering.rs` says it exactly:
//!
//! > **No ambient randomness.** `Shuffle` is a written-down seed and a xorshift, so a
//! > failure here reproduces exactly (INV-005 does not stop at `src/`).
//!
//! What that file lacks — and what this bone's acceptance criteria name — is *shrinking*
//! and *retention*: a refuted law must reduce to a minimal counterexample, and that
//! counterexample must be written down and replay on its own.
//!
//! # Why one file, included by `#[path]` from four crates
//!
//! A Rust integration test is its own crate, so test-only code is shared either through a
//! library crate or through a path include. A new crate is not available to a
//! test-addition bone: `crates/*` is the plan §20 member list, enforced by
//! `tools/check_crate_boundaries.py`, and every edge needs a
//! `dependency-rationale.toml` entry. So this file lives once, in the leaf crate every
//! other suite's crate already depends on, and the suites in `continuum-workspace`,
//! `continuum-context`, and `continuumd` include it with
//!
//! ```text
//! #[path = "../../continuum-value/tests/support/property.rs"]
//! mod property;
//! ```
//!
//! Nothing in this file mentions `continuum_value`, so the include carries no dependency:
//! it is text, compiled into whichever test binary asked for it. The precedent for the
//! shape — a `tests/support` module compiled into several binaries, `#![allow(dead_code)]`
//! because no one binary uses all of it — is
//! `crates/continuum-benchmark/tests/support/mod.rs`.
//!
//! # The contract a suite gets
//!
//! 1. [`Prng`] — splitmix64 over a written-down seed. No other entropy exists here.
//! 2. [`Domain`] — generate, shrink, and measure one kind of case.
//! 3. [`check`] — run a law over `plan.cases` generated cases; on the first refutation,
//!    shrink it to a local minimum and report both.
//! 4. [`Sexp`] — the textual counterexample form. Atoms are hex, so a counterexample is
//!    ASCII, diffable, and free of the quoting questions a `Debug` rendering raises.
//! 5. [`replay`] — run a law against a counterexample parsed from its text, with the
//!    generator out of the picture entirely. This is what "retained and replays
//!    deterministically" means operationally: the pinned string in a suite is not a
//!    comment about a past failure, it is an input the suite still runs.
//!
//! `crates/continuum-value/tests/phase_a_property_harness.rs` is this file's own evidence —
//! that a seed reproduces its draw, that the text form round trips, that a false law
//! shrinks to the exact boundary, and that the shrinker terminates against a hostile
//! `Domain`. It is a separate binary so the harness's self-evidence is counted once rather
//! than once per suite.
//!
//! # Termination
//!
//! [`Domain::shrink`] is free to propose anything; [`check`] accepts a candidate only when
//! [`Domain::size`] strictly decreases, and stops after [`MAX_SHRINK_STEPS`] accepted
//! candidates. Size is a `usize` bounded below by zero, so the loop terminates whether or
//! not a `shrink` implementation is well behaved. A test that can spin is not a test.
//!
//! # Bounded, linear inputs
//!
//! Every generator written over this harness draws a *width* and a *depth* from bounds it
//! declares up front, and the adversarial cases construct their input linearly
//! (`for _ in 0..n { push(one_more) }`) rather than by repeated doubling. The workspace has
//! an OOM in its history from a test that built its input by doubling; the bound to target
//! is the real one the code under test declares, and it is reached by counting to it.

// A shared `tests/support` module is compiled into every test binary that declares it, and
// no single binary uses every helper. The alternative is four copies of this file.
#![allow(dead_code)]

use core::fmt;

// --- the seeded source ----------------------------------------------------------------

/// A splitmix64 generator over a written-down seed.
///
/// splitmix64 rather than the xorshift `deterministic_ordering.rs` uses, for one reason:
/// xorshift64 has a fixed point at zero and a slow walk-up from small seeds, so a seed
/// written as `1` in a test source produces a visibly non-uniform first few draws.
/// splitmix64 is a bijection on `u64` with no fixed point, so every seed — including `0` —
/// is as good as every other, and a suite's seed can be the number the author felt like
/// writing rather than a value chosen to dodge a degenerate state.
///
/// The algorithm is fixed here, in full, and depends on nothing but its own state: no
/// clock, no environment, no address, no `std` RNG. Two runs of the same [`Plan`] on any
/// platform draw the same sequence, which is what makes a refutation reproducible and a
/// green run meaningful.
#[derive(Debug, Clone)]
pub struct Prng {
    state: u64,
}

impl Prng {
    /// A generator seeded with `seed`. The only constructor: entropy is an argument.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..bound`. A `bound` below two yields zero rather than panicking.
    pub fn below(&mut self, bound: usize) -> usize {
        if bound <= 1 {
            return 0;
        }
        let bound = u64::try_from(bound).unwrap_or(u64::MAX);
        usize::try_from(self.next_u64() % bound).unwrap_or(0)
    }

    /// A number in `low..=high`.
    pub fn between(&mut self, low: usize, high: usize) -> usize {
        if high <= low {
            return low;
        }
        low + self.below(high - low + 1)
    }

    /// A coin.
    pub fn flip(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    /// A byte.
    pub fn byte(&mut self) -> u8 {
        u8::try_from(self.next_u64() >> 56).unwrap_or(0)
    }

    /// One item of `items`, by index.
    ///
    /// # Panics
    ///
    /// When `items` is empty: an empty alphabet is a bug in the generator, not a case the
    /// property is about.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        assert!(!items.is_empty(), "a generator drew from an empty alphabet");
        let index = self.below(items.len());
        &items[index]
    }

    /// A deterministic Fisher-Yates permutation of `items`, in place.
    pub fn permute<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = self.below(index + 1);
            items.swap(index, other);
        }
    }
}

// --- the counterexample form ----------------------------------------------------------

/// Nesting a [`Sexp`] rendering or parse will accept.
///
/// Well above any counterexample these suites produce, and low enough that a hostile string
/// cannot drive the parser into the stack. The parser is recursive; the bound is what makes
/// that safe.
pub const MAX_SEXP_DEPTH: usize = 64;

/// The textual form a retained counterexample is written in.
///
/// An atom renders as `#` followed by lowercase hex; a list renders as its items in
/// parentheses, single-space separated. Hex rather than quoted text on purpose: a
/// counterexample is frequently a byte string that is not UTF-8, or text with a newline or
/// a quote in it, and a form that needs escaping needs an escaping *convention*, which is a
/// second spelling waiting to happen. Hex has one spelling, is ASCII, diffs cleanly, and
/// survives being pasted into a test source.
///
/// The empty atom is `#`; the empty list is `()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sexp {
    /// An opaque byte string.
    Atom(Vec<u8>),
    /// An ordered list.
    List(Vec<Sexp>),
}

impl Sexp {
    /// An atom over raw bytes.
    #[must_use]
    pub fn atom(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Atom(bytes.into())
    }

    /// An atom over a string's UTF-8 bytes.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self::Atom(text.as_bytes().to_vec())
    }

    /// An atom over the decimal rendering of `value`.
    #[must_use]
    pub fn number(value: u128) -> Self {
        Self::text(&value.to_string())
    }

    /// A list.
    #[must_use]
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self::List(items.into_iter().collect())
    }

    /// The bytes, when this is an atom.
    #[must_use]
    pub fn as_atom(&self) -> Option<&[u8]> {
        match self {
            Self::Atom(bytes) => Some(bytes),
            Self::List(_) => None,
        }
    }

    /// The atom's bytes as UTF-8, when this is an atom holding UTF-8.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        core::str::from_utf8(self.as_atom()?).ok()
    }

    /// The atom's bytes parsed as a decimal `u128`.
    #[must_use]
    pub fn as_number(&self) -> Option<u128> {
        self.as_text()?.parse().ok()
    }

    /// The items, when this is a list.
    #[must_use]
    pub fn as_list(&self) -> Option<&[Self]> {
        match self {
            Self::Atom(_) => None,
            Self::List(items) => Some(items),
        }
    }

    /// The items, when this is a list of exactly `N`.
    #[must_use]
    pub fn as_tuple<const N: usize>(&self) -> Option<&[Self; N]> {
        self.as_list()?.try_into().ok()
    }

    /// The canonical text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out);
        out
    }

    fn render_into(&self, out: &mut String) {
        match self {
            Self::Atom(bytes) => {
                out.push('#');
                for byte in bytes {
                    out.push(char::from(HEX[usize::from(byte >> 4)]));
                    out.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
            }
            Self::List(items) => {
                out.push('(');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(' ');
                    }
                    item.render_into(out);
                }
                out.push(')');
            }
        }
    }

    /// Read the canonical text back.
    ///
    /// Strict: trailing characters, an odd hex run, a non-hex digit, an unbalanced
    /// parenthesis, a doubled or misplaced separator, and nesting past [`MAX_SEXP_DEPTH`]
    /// all yield [`None`] rather than a best-effort tree. One text, one tree — the same
    /// discipline the encodings under test are held to. A retained counterexample that no
    /// longer parses is a failing test, not a skipped one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let mut at = 0usize;
        let sexp = parse_at(bytes, &mut at, 1)?;
        if at == bytes.len() { Some(sexp) } else { None }
    }
}

impl fmt::Display for Sexp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

const fn unhex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn parse_at(bytes: &[u8], at: &mut usize, depth: usize) -> Option<Sexp> {
    if depth > MAX_SEXP_DEPTH {
        return None;
    }
    match bytes.get(*at)? {
        b'#' => {
            *at += 1;
            let mut out = Vec::new();
            while let Some(high) = bytes.get(*at).copied().and_then(unhex) {
                let low = unhex(*bytes.get(*at + 1)?)?;
                out.push((high << 4) | low);
                *at += 2;
            }
            Some(Sexp::Atom(out))
        }
        b'(' => {
            *at += 1;
            let mut items: Vec<Sexp> = Vec::new();
            // `true` exactly when the last thing read was an item, so a separator is legal
            // here and another item is not. One flag rules out `( #00)`, `(#00  #01)`,
            // `(#00 )`, and `(#00#01)` in one place.
            let mut after_item = false;
            loop {
                match bytes.get(*at)? {
                    b')' => {
                        if !after_item && !items.is_empty() {
                            return None;
                        }
                        *at += 1;
                        return Some(Sexp::List(items));
                    }
                    b' ' => {
                        if !after_item {
                            return None;
                        }
                        after_item = false;
                        *at += 1;
                    }
                    _ => {
                        if after_item {
                            return None;
                        }
                        items.push(parse_at(bytes, at, depth + 1)?);
                        after_item = true;
                    }
                }
            }
        }
        _ => None,
    }
}

// --- cases and domains ----------------------------------------------------------------

/// One generated input, with a textual form that round trips.
///
/// The round trip is load-bearing rather than decorative: [`check`] renders the minimal
/// counterexample, a suite pins that string, and [`replay`] reads it back and re-runs the
/// law against it. If [`Case::from_sexp`] were lossy the retained corpus would hold a
/// different input than the one that failed, and the retention would prove nothing.
pub trait Case: Clone + fmt::Debug + Sized {
    /// This case as a tree.
    fn to_sexp(&self) -> Sexp;

    /// The case a tree denotes, or [`None`] when the tree is not one.
    fn from_sexp(sexp: &Sexp) -> Option<Self>;

    /// The canonical text.
    fn repr(&self) -> String {
        self.to_sexp().render()
    }

    /// The case a canonical text denotes.
    fn from_repr(text: &str) -> Option<Self> {
        Self::from_sexp(&Sexp::parse(text)?)
    }
}

/// One kind of input: how to draw it, how to make it smaller, and how big it is.
pub trait Domain {
    /// The input this domain produces.
    type Item: Case;

    /// Draw one case. Must be a pure function of `rng`'s state.
    fn generate(&self, rng: &mut Prng) -> Self::Item;

    /// Candidate smaller cases, in a fixed order.
    ///
    /// Order is the shrinker's whole strategy — the first candidate that still refutes the
    /// law is taken — so an implementation puts its most aggressive reduction first. There
    /// is no obligation to return only smaller cases: [`check`] filters by [`Domain::size`]
    /// and would otherwise not terminate.
    fn shrink(&self, item: &Self::Item) -> Vec<Self::Item>;

    /// How big a case is. Strictly decreasing under an accepted shrink.
    fn size(&self, item: &Self::Item) -> usize;
}

/// A pair of cases from one domain: the shape an antisymmetry or an injectivity law needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair<T> {
    /// The left case.
    pub left: T,
    /// The right case.
    pub right: T,
}

impl<T: Case> Case for Pair<T> {
    fn to_sexp(&self) -> Sexp {
        Sexp::list([self.left.to_sexp(), self.right.to_sexp()])
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        let [left, right] = sexp.as_tuple::<2>()?;
        Some(Self {
            left: T::from_sexp(left)?,
            right: T::from_sexp(right)?,
        })
    }
}

/// [`Pair`] over an inner domain.
#[derive(Debug, Clone)]
pub struct Pairs<D>(pub D);

impl<D: Domain> Domain for Pairs<D> {
    type Item = Pair<D::Item>;

    fn generate(&self, rng: &mut Prng) -> Self::Item {
        Pair {
            left: self.0.generate(rng),
            right: self.0.generate(rng),
        }
    }

    fn shrink(&self, item: &Self::Item) -> Vec<Self::Item> {
        shrink_pair(&self.0, item)
    }

    fn size(&self, item: &Self::Item) -> usize {
        self.0.size(&item.left) + self.0.size(&item.right)
    }
}

/// [`Pair`] over an inner domain, where the right half is often a *neighbour* of the left.
///
/// Independent draws are the wrong corpus for an injectivity law. Two unrelated cases
/// almost always differ in several ways at once, so an implementation that ignores one
/// field is separated by the fields it does read, and the bug survives an arbitrarily large
/// corpus of independent pairs. What catches it is a pair that differs in *exactly* the
/// ignored field — and the domain already knows how to produce those, because that is what
/// [`Domain::shrink`] enumerates: structurally adjacent cases, one edit away.
///
/// So half the draws take the right half from `shrink(left)`. The other half stay
/// independent, because an injectivity law also has to hold of unrelated cases and a corpus
/// of nothing but neighbours would be a different, narrower claim.
#[derive(Debug, Clone)]
pub struct NearPairs<D>(pub D);

impl<D: Domain> Domain for NearPairs<D> {
    type Item = Pair<D::Item>;

    fn generate(&self, rng: &mut Prng) -> Self::Item {
        let left = self.0.generate(rng);
        let neighbours = self.0.shrink(&left);
        let right = if rng.flip() || neighbours.is_empty() {
            self.0.generate(rng)
        } else {
            let index = rng.below(neighbours.len());
            neighbours[index].clone()
        };
        Pair { left, right }
    }

    fn shrink(&self, item: &Self::Item) -> Vec<Self::Item> {
        shrink_pair(&self.0, item)
    }

    fn size(&self, item: &Self::Item) -> usize {
        self.0.size(&item.left) + self.0.size(&item.right)
    }
}

/// Shrink a pair: the same edit on both halves first, then each half on its own.
///
/// The joint step matters more than it looks. A law about a *pair* — injectivity, an order
/// law, an identity that must separate two artifacts — usually fails only while the two
/// halves stay aligned in everything except the one difference that matters. Reducing one
/// half alone breaks the alignment, the law starts holding, the candidate is rejected, and
/// the shrinker stops on a counterexample still carrying every piece of shared noise the
/// generator happened to draw. Applying the *i*-th reduction to both halves at once keeps
/// them aligned, so the shared noise comes off and what remains is the difference itself.
fn shrink_pair<D: Domain>(domain: &D, item: &Pair<D::Item>) -> Vec<Pair<D::Item>> {
    let left_candidates = domain.shrink(&item.left);
    let right_candidates = domain.shrink(&item.right);
    let mut out = Vec::new();
    // The diagonal — the *i*-th reduction on both halves — first, because when the two
    // halves have the same shape it is the reduction that keeps them aligned.
    for (left, right) in left_candidates.iter().zip(right_candidates.iter()) {
        out.push(Pair {
            left: left.clone(),
            right: right.clone(),
        });
    }
    // Then the rest of the joint square, capped. The diagonal only aligns when the two
    // candidate lists happen to be the same length and in the same order, which is exactly
    // what stops being true once the halves differ — so the general "some reduction on the
    // left, some reduction on the right" is what actually removes shared noise. The cap
    // keeps the work per shrink step bounded and the enumeration order fixed.
    for left in left_candidates.iter().take(JOINT_SHRINK_CAP) {
        for right in right_candidates.iter().take(JOINT_SHRINK_CAP) {
            out.push(Pair {
                left: left.clone(),
                right: right.clone(),
            });
        }
    }
    for left in left_candidates {
        out.push(Pair {
            left,
            right: item.right.clone(),
        });
    }
    for right in right_candidates {
        out.push(Pair {
            left: item.left.clone(),
            right,
        });
    }
    out
}

/// A triple of cases from one domain: the shape a transitivity law needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triple<T> {
    /// The first case.
    pub first: T,
    /// The second case.
    pub second: T,
    /// The third case.
    pub third: T,
}

impl<T: Case> Case for Triple<T> {
    fn to_sexp(&self) -> Sexp {
        Sexp::list([
            self.first.to_sexp(),
            self.second.to_sexp(),
            self.third.to_sexp(),
        ])
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        let [first, second, third] = sexp.as_tuple::<3>()?;
        Some(Self {
            first: T::from_sexp(first)?,
            second: T::from_sexp(second)?,
            third: T::from_sexp(third)?,
        })
    }
}

/// [`Triple`] over an inner domain.
#[derive(Debug, Clone)]
pub struct Triples<D>(pub D);

impl<D: Domain> Domain for Triples<D> {
    type Item = Triple<D::Item>;

    fn generate(&self, rng: &mut Prng) -> Self::Item {
        Triple {
            first: self.0.generate(rng),
            second: self.0.generate(rng),
            third: self.0.generate(rng),
        }
    }

    fn shrink(&self, item: &Self::Item) -> Vec<Self::Item> {
        let mut out = Vec::new();
        // The joint step, for the reason given on `shrink_pair`.
        let firsts = self.0.shrink(&item.first);
        let seconds = self.0.shrink(&item.second);
        let thirds = self.0.shrink(&item.third);
        for ((first, second), third) in firsts.iter().zip(seconds.iter()).zip(thirds.iter()) {
            out.push(Triple {
                first: first.clone(),
                second: second.clone(),
                third: third.clone(),
            });
        }
        for first in firsts.iter().take(JOINT_SHRINK_CAP) {
            for second in seconds.iter().take(JOINT_SHRINK_CAP) {
                for third in thirds.iter().take(JOINT_SHRINK_CAP) {
                    out.push(Triple {
                        first: first.clone(),
                        second: second.clone(),
                        third: third.clone(),
                    });
                }
            }
        }
        for first in self.0.shrink(&item.first) {
            out.push(Triple {
                first,
                second: item.second.clone(),
                third: item.third.clone(),
            });
        }
        for second in self.0.shrink(&item.second) {
            out.push(Triple {
                first: item.first.clone(),
                second,
                third: item.third.clone(),
            });
        }
        for third in self.0.shrink(&item.third) {
            out.push(Triple {
                first: item.first.clone(),
                second: item.second.clone(),
                third,
            });
        }
        out
    }

    fn size(&self, item: &Self::Item) -> usize {
        self.0.size(&item.first) + self.0.size(&item.second) + self.0.size(&item.third)
    }
}

// --- running a law --------------------------------------------------------------------

/// The most shrink steps [`check`] will accept before reporting what it has.
///
/// A ceiling, not a target: every [`Domain`] in these suites reaches a local minimum in far
/// fewer. It exists so that a `shrink` implementation whose `size` disagrees with its
/// candidates still terminates with a report instead of running until the test times out.
pub const MAX_SHRINK_STEPS: usize = 512;

/// How many candidates from each half a [`Pair`] or [`Triple`] shrink will combine.
///
/// The joint square is quadratic (cubic for a triple) in this number, so it is a small
/// one: enough that the usual handful of reductions a domain offers are all combined, and
/// small enough that one shrink step stays a bounded amount of work no matter how many
/// candidates a domain proposes.
pub const JOINT_SHRINK_CAP: usize = 8;

/// One law, one seed, one case count.
///
/// The seed is written in the test source and nowhere else. Two runs of the same plan draw
/// the same cases; changing the seed is an edit with a diff, not a re-roll.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// The law's name, as it appears in a failure report.
    pub law: &'static str,
    /// The seed the generator starts from.
    pub seed: u64,
    /// How many cases to draw.
    pub cases: usize,
}

impl Plan {
    /// A plan.
    #[must_use]
    pub const fn new(law: &'static str, seed: u64, cases: usize) -> Self {
        Self { law, seed, cases }
    }
}

/// What a refuted law leaves behind.
#[derive(Debug, Clone)]
pub struct Refutation<T> {
    /// Which drawn case refuted the law, counting from zero.
    pub case_index: usize,
    /// The case as it was drawn.
    pub generated: T,
    /// [`Domain::size`] of the drawn case.
    pub generated_size: usize,
    /// The local minimum the shrinker reached.
    pub minimal: T,
    /// [`Domain::size`] of the minimum.
    pub minimal_size: usize,
    /// [`Case::repr`] of the minimum: the string a suite pins and [`replay`] reads.
    pub minimal_repr: String,
    /// How many candidates the shrinker accepted.
    pub shrink_steps: usize,
    /// What the law said about the minimum.
    pub reason: String,
}

/// What running a law over a plan produced.
#[derive(Debug, Clone)]
pub enum Report<T> {
    /// No drawn case refuted the law.
    Held {
        /// How many cases were drawn.
        cases: usize,
    },
    /// A case refuted the law, and here is the smallest one the shrinker found.
    Refuted(Box<Refutation<T>>),
}

impl<T: Case> Report<T> {
    /// Assert the law held; panic with the minimal counterexample when it did not.
    ///
    /// The panic message carries [`Refutation::minimal_repr`], which is the exact string a
    /// regression test pins — so the first thing a failure hands the reader is the input it
    /// should be replayed against.
    ///
    /// # Panics
    ///
    /// When the law was refuted.
    pub fn expect_held(self, plan: &Plan) -> usize {
        match self {
            Self::Held { cases } => cases,
            Self::Refuted(refutation) => panic!(
                "law `{law}` (seed {seed}, {cases} cases) was refuted by case #{index}\n  \
                 minimal counterexample: {repr}\n  \
                 minimal case          : {minimal:?}\n  \
                 shrunk                : size {from} -> {to} in {steps} steps\n  \
                 reason                : {reason}",
                law = plan.law,
                seed = plan.seed,
                cases = plan.cases,
                index = refutation.case_index,
                repr = refutation.minimal_repr,
                minimal = refutation.minimal,
                from = refutation.generated_size,
                to = refutation.minimal_size,
                steps = refutation.shrink_steps,
                reason = refutation.reason,
            ),
        }
    }

    /// Assert the law was refuted, and hand back the refutation; panic when it held.
    ///
    /// This is the anti-vacuity direction. A suite runs its own law against a deliberately
    /// broken seam and asserts *this*: a suite that cannot fail is not evidence, and a
    /// suite that is merely *said* to be able to fail is a claim about a suite.
    ///
    /// # Panics
    ///
    /// When the law held — which means the suite cannot detect the seeded violation.
    pub fn expect_refuted(self, plan: &Plan) -> Refutation<T> {
        match self {
            Self::Held { cases } => panic!(
                "law `{law}` (seed {seed}) was expected to be refuted by its seeded \
                 violation, but held over all {cases} cases: the suite cannot fail, so a \
                 green run of it is not evidence",
                law = plan.law,
                seed = plan.seed,
            ),
            Self::Refuted(refutation) => *refutation,
        }
    }
}

/// Run `law` over `plan.cases` cases drawn from `domain`, shrinking the first refutation.
///
/// The law answers `Ok(())` when it holds of a case and `Err(reason)` when it does not.
/// `reason` is prose for a human; nothing branches on it.
pub fn check<D, L>(plan: &Plan, domain: &D, law: L) -> Report<D::Item>
where
    D: Domain,
    L: Fn(&D::Item) -> Result<(), String>,
{
    let mut rng = Prng::new(plan.seed);
    for case_index in 0..plan.cases {
        let generated = domain.generate(&mut rng);
        if law(&generated).is_ok() {
            continue;
        }
        let generated_size = domain.size(&generated);
        let (minimal, shrink_steps) = shrink_to_minimum(domain, &law, generated.clone());
        let reason = law(&minimal)
            .err()
            .unwrap_or_else(|| "the shrunk case no longer refutes the law".to_owned());
        return Report::Refuted(Box::new(Refutation {
            case_index,
            generated,
            generated_size,
            minimal_size: domain.size(&minimal),
            minimal_repr: minimal.repr(),
            minimal,
            shrink_steps,
            reason,
        }));
    }
    Report::Held { cases: plan.cases }
}

/// Greedy descent: take the first strictly smaller candidate that still refutes.
fn shrink_to_minimum<D, L>(domain: &D, law: &L, start: D::Item) -> (D::Item, usize)
where
    D: Domain,
    L: Fn(&D::Item) -> Result<(), String>,
{
    let mut current = start;
    let mut steps = 0usize;
    while steps < MAX_SHRINK_STEPS {
        let current_size = domain.size(&current);
        let Some(next) = domain
            .shrink(&current)
            .into_iter()
            .find(|candidate| domain.size(candidate) < current_size && law(candidate).is_err())
        else {
            break;
        };
        current = next;
        steps += 1;
    }
    (current, steps)
}

/// Re-run `law` against a case parsed from its retained text.
///
/// No [`Prng`], no [`Domain`], no seed: a retained counterexample is an input, and this is
/// the whole path from the pinned string to the verdict.
///
/// # Panics
///
/// When the text does not parse as a case. A retained counterexample that has stopped being
/// readable is a regression in the harness and must not be silently skipped.
pub fn replay<C, L>(repr: &str, law: L) -> Result<(), String>
where
    C: Case,
    L: Fn(&C) -> Result<(), String>,
{
    let case = C::from_repr(repr)
        .unwrap_or_else(|| panic!("a retained counterexample no longer parses: {repr}"));
    law(&case)
}

/// Assert a retained counterexample still refutes `law`, and hand back the reason.
///
/// The companion of [`Report::expect_refuted`]: that one proves the *suite* can fail, this
/// one proves the *retained case* is still the case that fails it, reached without drawing
/// anything.
///
/// # Panics
///
/// When the retained case no longer refutes its law.
pub fn expect_replay_refutes<C, L>(repr: &str, law: L) -> String
where
    C: Case,
    L: Fn(&C) -> Result<(), String>,
{
    match replay::<C, L>(repr, law) {
        Ok(()) => panic!(
            "the retained counterexample `{repr}` no longer refutes its law: either the law \
             changed or the seeded violation was repaired, and the retention is now stale"
        ),
        Err(reason) => reason,
    }
}
