//! The persisted corpus and its manifest.
//!
//! # Layout
//!
//! ```text
//! crates/continuum-cml-elab/tests/cml-fuzz-corpus/
//! ├── MANIFEST                one line per entry: where every target lands on it
//! └── cases/
//!     ├── <class>-<name>.ctm   hand-written adversarial cases (cyclic, oversized, …)
//!     ├── derived-*.ctm        mutants drawn from the declared derivation seeds
//!     └── regression-*.ctm     minimized findings (none yet)
//! ```
//!
//! The seeds are the repository's own CML sources ([`SEEDS`]), read in place so a change
//! to them is a change to this corpus. The derived cases are a pure function of the
//! generator, the seeds, the hand-written cases, and [`DERIVE_SEEDS`], and a test checks
//! the committed files are exactly that function's output.
//!
//! # The manifest
//!
//! Each line is `<entry>` then, per target, `<target>=<landing>`, with `@<fingerprint>`
//! appended when the target accepted. The fingerprint is of the normalized identity and
//! dump (elaborate), of `Model::identity` (lower), or of the model and run identities
//! (lower under a configuration). The manifest is written by one process and checked by
//! every later one, so it is the cross-process half of the determinism law: another
//! process, with other hash seeds, another address layout, and another thread schedule,
//! must reproduce every identity byte for byte.
//!
//! Regenerate with `CML_FUZZ_BLESS=1 cargo test -p continuum-cml-elab --test cml_fuzz`,
//! and review the diff.

use std::path::{Path, PathBuf};

use super::generate::{Rng, generate};

/// The repository's CML sources, repository-relative. Every `.ctm` in the tree.
pub const SEEDS: &[&str] = &[
    "notes/plan/examples/replicated_register.ctm",
    "notes/plan/examples/forge_ack_protocol.ctm",
    "notes/plan/corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm",
    "notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm",
    "crates/continuum-cml-elab/tests/configs/Ring.ctm",
    "crates/continuum-cml-syntax/tests/fixtures/CoffeeCan.ctm",
    "crates/continuum-cml-syntax/tests/fixtures/SyntaxCoverage.ctm",
    "crates/continuum-cml-syntax/tests/fixtures/TowerOfHanoi.ctm",
    "crates/continuum-cml-syntax/tests/fixtures/TransitiveClosure.ctm",
];

/// The seeds the derived cases are drawn from, and how many per seed.
pub const DERIVE_SEEDS: [u64; 4] = [0x1a4a, 0x00c0_ffee, 0x5eed_c0de, 0x9e37_79b9_7f4a_7c15];
/// Derived cases per derivation seed.
pub const DERIVE_PER_SEED: usize = 8;

/// Where an entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A repository source, read in place.
    Seed,
    /// A hand-written adversarial case.
    Hand,
    /// A mutant drawn from [`DERIVE_SEEDS`].
    Derived,
    /// A minimized finding.
    Regression,
}

/// One corpus entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// `seed/<path>` or `cases/<file>`.
    pub name: String,
    /// The source text.
    pub source: String,
    /// Where it came from.
    pub origin: Origin,
}

/// The repository root.
#[must_use]
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The corpus directory.
#[must_use]
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cml-fuzz-corpus")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The seeds, in [`SEEDS`] order.
#[must_use]
pub fn seeds() -> Vec<Entry> {
    SEEDS
        .iter()
        .map(|rel| Entry {
            name: format!("seed/{rel}"),
            source: read(&repo().join(rel)),
            origin: Origin::Seed,
        })
        .collect()
}

/// The committed cases, sorted by file name.
#[must_use]
pub fn cases() -> Vec<Entry> {
    let mut names: Vec<String> = std::fs::read_dir(dir().join("cases"))
        .expect("the corpus has a cases directory")
        .map(|e| {
            e.expect("a directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .filter(|n| n.ends_with(".ctm"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|file| {
            let origin = if file.starts_with("derived-") {
                Origin::Derived
            } else if file.starts_with("regression-") {
                Origin::Regression
            } else {
                Origin::Hand
            };
            Entry {
                source: read(&dir().join("cases").join(&file)),
                name: format!("cases/{file}"),
                origin,
            }
        })
        .collect()
}

/// Seeds then committed cases.
#[must_use]
pub fn all() -> Vec<Entry> {
    let mut out = seeds();
    out.extend(cases());
    out
}

/// The derived cases this build declares: `(file name, source)`.
///
/// Drawn from the seeds and the hand-written cases only, never from derived or
/// regression cases, so the derivation does not feed on its own output.
#[must_use]
pub fn derived() -> Vec<(String, String)> {
    let hosts: Vec<String> = all()
        .into_iter()
        .filter(|e| matches!(e.origin, Origin::Seed | Origin::Hand))
        .map(|e| e.source)
        .collect();
    let mut out = Vec::new();
    for seed in DERIVE_SEEDS {
        let mut rng = Rng::new(seed);
        for i in 0..DERIVE_PER_SEED {
            out.push((
                format!("derived-{seed:016x}-{i}.ctm"),
                generate(&mut rng, &hosts),
            ));
        }
    }
    out
}

/// The manifest path.
#[must_use]
pub fn manifest_path() -> PathBuf {
    dir().join("MANIFEST")
}

/// The committed manifest text.
#[must_use]
pub fn manifest() -> String {
    read(&manifest_path())
}

/// Whether this run regenerates the committed files instead of checking them.
#[must_use]
pub fn blessing() -> bool {
    std::env::var_os("CML_FUZZ_BLESS").is_some()
}
