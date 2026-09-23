//! Generated finite semantic systems and the tiny exhaustive oracle (docs/19 §2,
//! bn-1zgs under the bn-3ly0 semantic-differential harness).
//!
//! > A small generator creates finite systems with: typed state variables; guarded
//! > transitions; explicit read/write footprints; independent/dependent pairs;
//! > conflicts; obligations; cancellation phases; fairness annotations. For small
//! > sizes, enumerate all interleavings and configurations. Compare every optimized
//! > engine and reduction against this oracle.
//! >
//! > — `notes/plan/docs/19_TEST_STRATEGY.md` §2
//!
//! docs/19 §1 places this layer — "differential tiny exhaustive tests" — between the
//! generated litmus tests and the metamorphic suite, and §5 names the pairing it
//! exists for: "exhaustive enumerator versus DPOR". RFC 0013 ("Generated finite
//! universes") asks for the same thing at the operator level. The oracle is the
//! reference the later DPOR and reduction engines (`continuum-engine-dpor`, RFC 0014)
//! are checked against, so its semantics is stated here in full, and the code in
//! [`oracle`] implements exactly this and nothing more.
//!
//! # Where it lives, and why
//!
//! In the reference engine, because the reference path is already "the differential
//! oracle every optimized path is measured against" (this crate's root documentation)
//! and because every piece it needs is here: [`crate::model::Model`] for the typed
//! variables and guarded actions, [`crate::bfs::explore`] for the closed reachable set,
//! and [`crate::witness::shortest`] for witnesses. Nothing is duplicated; a generated
//! system *is* a `Model` plus the declarations in [`system`]. It is quality
//! infrastructure, not trusted checking: no `continuum-kernel-*` or
//! `continuum-certificate` crate may depend on this crate (plan §20;
//! `tools/check_crate_boundaries.py`), and it adds no dependency to it.
//!
//! # The semantics
//!
//! A system `Σ = (M, K, Fp, Cf, Fair, Ob, Ph)` is a model `M` (variables with finite
//! domains, deterministic guarded actions, initial states) with a kind `K(v)` per
//! variable, a declared footprint `Fp(a) = (R_a, W_a)` per action, a symmetric set of
//! declared conflicts `Cf`, a fairness assumption `Fair(a) ∈ {unfair, weak}` per
//! action, obligations `Ob` (an obligation `o` is open in `s` iff `s(var_o) ≠ 0`,
//! optionally owned by a phase variable), and phase variables `Ph` with final values.
//!
//! Write `post_a(s)` for the unique successor of `s` under `a` when `a` is enabled in
//! `s`. `Reach` is the set of states reachable from the initial states, computed to
//! closure. Every clause below quantifies over `Reach` only.
//!
//! 1. **Configurations.** The artifact lists `Reach` in ascending order with each
//!    state's breadth-first depth, every edge `s -a-> post_a(s)` with `s ∈ Reach`,
//!    and the quiescent states (no enabled action).
//! 2. **Semantic dependence.** Distinct `a`, `b` are *dependent* iff some `s ∈ Reach`
//!    enables both and the diamond fails: `b` is disabled in `post_a(s)`, or `a` is
//!    disabled in `post_b(s)`, or `post_b(post_a(s)) ≠ post_a(post_b(s))`. This is
//!    docs/02 §3's diamond obligation together with enabledness preservation (the
//!    independence of Flanagan and Godefroid, POPL 2005, restricted to `Reach`). The
//!    artifact lists every dependent pair.
//! 3. **Claimed independence** is `a ≠ b`, `Fp(a)` and `Fp(b)` do not interfere
//!    (`W_a ∩ (R_b ∪ W_b) = ∅` and `W_b ∩ R_a = ∅`), and `{a, b} ∉ Cf`. A
//!    *dependence defect* is any of: a claimed-independent pair that is semantically
//!    dependent; a variable some edge of `a` changes that is not in `W_a`; a variable
//!    `a`'s guard or update syntax mentions that is not in `R_a`.
//! 4. **Obligation defects.** For each obligation `o`: a quiescent state in `Reach`
//!    with `o` open (*quiescence leak*); a state in `Reach` where `o`'s owner phase has
//!    its final value and `o` is open (*completion leak*, docs/02 §7: Cancelled only
//!    when `obligations == ∅`).
//! 5. **Fairness defects.** An infinite execution is *weakly fair* iff no weakly fair
//!    action is, from some point on, continuously enabled and never taken (docs/02 §8
//!    `WeakFair`). A fairness defect for `o` is a weakly fair infinite execution along
//!    which `o` is eventually always open. Over a finite graph this holds iff some
//!    strongly connected component `C` of `Reach` restricted to states with `o` open
//!    has an internal edge and, for every weakly fair `a`, some state of `C` disables
//!    `a` or some internal edge of `C` is labelled `a`. The witness is a lasso: a
//!    shortest stem and a cycle inside `C` that visits each such site.
//! 6. **Cancellation defects.** An edge that lowers a phase variable.
//! 7. **Interleavings.** Every execution prefix from each initial state of at most
//!    `max_depth` steps that is either *complete* (ends quiescent) or *truncated*
//!    (exactly `max_depth` steps with an action still enabled) is enumerated and
//!    counted. Their Mazurkiewicz classes under the dependence relation of clause 2
//!    are counted by Foata normal form (Cartier–Foata; docs/13). Because clause 2
//!    quantifies over all of `Reach`, two words in one class from one initial state
//!    reach one state. This is the relation a reduction is checked against: a
//!    static independence relation is sound iff it is contained in the complement of
//!    clause 2, and a reduction using a sound static relation explores at least this
//!    many classes to the same depth. Fewer means it treated a dependent pair as
//!    independent. (A reduction with *conditional*, state-dependent independence is
//!    outside this comparison and must be checked against clause 2 directly.)
//!
//! Each finding records the first site in `(depth, state)` order with a shortest
//! witness, and at most one finding per (check, subject). The **verdict** is `clean`
//! iff there is no finding. Because `Reach` is closed, a clean verdict covers every
//! reachable configuration; only clause 7's counts are depth-bounded, and the
//! artifact carries the bound.
//!
//! # Limits and refusals
//!
//! [`oracle::Limits`] bounds variables, actions, the *declared* state space (the
//! product of all domains, so an accepted instance always explores to closure), the
//! interleaving depth, and the number of interleavings. Above a limit the oracle
//! returns [`oracle::Refusal::TooLarge`] naming the measure; an action with several
//! outcomes is [`oracle::Refusal::NondeterministicAction`]. These are distinct typed
//! outcomes (INV-008), and a refusal is never a partial artifact.
//!
//! # The canonical artifact
//!
//! [`oracle::Artifact::canonical_bytes`] is ASCII, LF-terminated, and a pure function
//! of the system and the limits: every collection is a `BTreeMap`/`BTreeSet` or a
//! sorted vector, and nothing reads a clock, an environment, or ambient entropy
//! (INV-005). The bytes embed the system's own canonical encoding, so the artifact is
//! self-describing. Under ADR-0013 the canonical bytes are the content identity;
//! a digest is only an index a store may add.
//!
//! # Seeded defects and shrinking
//!
//! [`defect::seed_defect`] applies one docs/19 §4 mutation operator per class, and
//! [`defect::shrink`] reduces a defective system to a 1-minimal one that still shows
//! every kind of finding of that class, keeping the mutation sites. [`generate::generate`] documents what the generator guarantees so that
//! each class has a site; the tests confirm each guarantee with the oracle rather than
//! assume it.

pub mod defect;
pub mod generate;
pub mod oracle;
pub mod prng;
pub mod system;

pub use defect::{SeedError, Seeded, ShrinkError, Shrunk, seed_defect, shrink};
pub use generate::{GenerateError, Shape, generate};
pub use oracle::{
    Artifact, Breach, DefectClass, Finding, Interleavings, Limits, Measure, Refusal, Trace,
    Verdict, run,
};
pub use prng::SplitMix64;
pub use system::{
    ActionMeta, Fairness, Footprint, ObligationDecl, PhaseDecl, Role, System, SystemError,
    SystemParts, VarKind,
};
