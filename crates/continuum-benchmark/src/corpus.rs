//! The Phase A corpus: two ports, their bytes, and the identity a daemon keys them by.
//!
//! # What a "source" is here
//!
//! A [`Source`] is one corpus port — a `.ctm` module, the companion files that travel in a
//! snapshot with it, and the [`Model`] this workspace can construct from it. The bytes are
//! `include_str!`'d from `notes/plan/corpus/tla-examples/ports/`, so the benchmark's inputs
//! are the dossier's own files and not a copy that can drift.
//!
//! # The source hash, and why it is the daemon's own
//!
//! Plan §19.4 partitions a dataset by "semantic families **and source hashes**", and
//! [`Source::source_hash`] computes that hash with [`model_source`] — the same function
//! `verification.start` uses to decide which registered model a snapshot resolves to. Two
//! consequences, both wanted:
//!
//! - the separation check and the daemon agree on what "the same source" means, because it
//!   is one function rather than two;
//! - a task whose `.ctm` bytes differ by one byte has a different source hash *and* resolves
//!   to no registered model, so leakage and unsupported-model detection are the same
//!   identity seen from two sides.
//!
//! [`Model`]: continuum_engine_reference::model::Model

use continuum_engine_reference::{diehard, model::Model, model::ModelError};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::verification::model_source;
use continuumd::protocol::scalar::Commitment;

use crate::philosophers;

/// TV-009's CML module (`notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`).
pub const DIE_HARD_MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
/// TV-009's model configuration.
pub const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
/// TV-009's port note.
pub const DIE_HARD_README: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/README.md");

/// TV-007's CML module
/// (`notes/plan/corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm`).
pub const PHILOSOPHERS_MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm");
/// TV-007's port note, which is where its frozen facts are written down.
pub const PHILOSOPHERS_README: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-007/README.md");

/// The governing Intent Contract every task in this benchmark runs under.
///
/// One contract for both ports, because the intent is not what the benchmark varies: the
/// two arms differ in *interface*, and holding intent, snapshot shape and budget fixed is
/// what makes the comparison paired (plan §24.5's ratified margins: "an identical base
/// model, task set, and per-task budget").
pub const CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// One corpus port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// TV-009 — Die Hard, the finite-safety seed.
    DieHard,
    /// TV-007 — Dining Philosophers, the deadlock seed.
    Philosophers,
}

impl Source {
    /// Every source, in report order.
    pub const ALL: [Self; 2] = [Self::DieHard, Self::Philosophers];

    /// The corpus identifier this port carries.
    #[must_use]
    pub const fn port(self) -> &'static str {
        match self {
            Self::DieHard => "TV-009",
            Self::Philosophers => "TV-007",
        }
    }

    /// The module's path inside a snapshot.
    #[must_use]
    pub const fn module_path(self) -> &'static str {
        match self {
            Self::DieHard => "DieHard.ctm",
            Self::Philosophers => "DiningPhilosophers.ctm",
        }
    }

    /// The module's bytes.
    #[must_use]
    pub const fn module(self) -> &'static str {
        match self {
            Self::DieHard => DIE_HARD_MODULE,
            Self::Philosophers => PHILOSOPHERS_MODULE,
        }
    }

    /// The non-module files that travel in a snapshot with it, in path order.
    ///
    /// TV-009 carries a model configuration and TV-007 does not, which is a fact about the
    /// corpus rather than a choice made here: manufacturing a configuration for TV-007 so
    /// the two snapshots looked alike would have put a file in a snapshot that the dossier
    /// does not have.
    #[must_use]
    pub const fn companions(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::DieHard => &[("README.md", DIE_HARD_README)],
            Self::Philosophers => &[("README.md", PHILOSOPHERS_README)],
        }
    }

    /// The configuration files, in path order.
    #[must_use]
    pub const fn configuration(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::DieHard => &[("default.model.toml", DIE_HARD_CONFIG)],
            Self::Philosophers => &[],
        }
    }

    /// The target identifier `verification.start` names for this port.
    #[must_use]
    pub const fn target_id(self) -> &'static str {
        match self {
            Self::DieHard => "DieHard",
            Self::Philosophers => "DiningPhilosophers",
        }
    }

    /// The model a daemon registers for this source.
    ///
    /// # Errors
    ///
    /// [`ModelError`] when the declaration is rejected. Die Hard's is
    /// `continuum-engine-reference`'s own; Dining Philosophers' is
    /// [`crate::philosophers`]'s, declared in that module's own vocabulary for the reason
    /// its documentation gives.
    pub fn model(self) -> Result<Model, ModelError> {
        match self {
            Self::DieHard => diehard::model(),
            Self::Philosophers => philosophers::model(),
        }
    }

    /// The content identity of this port's module set — plan §19.4's "source hash".
    ///
    /// # Panics
    ///
    /// Never for a non-empty module set: `model_source` returns [`None`] only when it is
    /// handed no modules, and every source here has exactly one.
    #[must_use]
    pub fn source_hash(self) -> Commitment {
        model_source(
            &Blake3Identity,
            [(self.module_path(), self.module().as_bytes())],
        )
        .expect("a source with one module has a content identity")
    }
}
