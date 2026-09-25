//! The one adapter seam an engine plugs in through.
//!
//! An engine is anything that answers questions about a `continuum-model-core`
//! [`Model`] under a declared [`Budget`]. The harness never links an engine: an adapter
//! implementing [`Engine`] is written where the engine is a dependency (for the engines
//! on trunk, this crate's tests over dev edges), and it projects the engine's own
//! answer onto [`Normalized`]. A new engine (the explicit-state engine, DPOR) joins by
//! writing one adapter and naming its slot in [`super::claims::LANES`].

use continuum_model_core::model::Model;

use super::normal::Normalized;

/// The reference path (`continuum-engine-reference`: `bfs`, `checking`, `witness`).
pub const REFERENCE: &str = "continuum-engine-reference";
/// The reference engine's tiny exhaustive oracle (`continuum-engine-reference::semantic`,
/// bn-1zgs). Its reachable set comes from the same `bfs::explore`, so on that field it
/// is a second reading, not a second search. Its quiescent-state computation is its
/// own.
pub const SEMANTIC_ORACLE: &str = "continuum-engine-reference::semantic";
/// The trusted checking base's finite-closure checker (`continuum-kernel-core`),
/// reached through wire-epoch-2 certificates: its model decoder and evaluator share no
/// code with `continuum-model-core`.
pub const KERNEL: &str = "continuum-kernel-core";
/// This harness's own closure oracle ([`super::closure`]): an independent search over
/// the model core's successor relation.
pub const CLOSURE: &str = "continuum-corpus::closure";
/// The optimized explicit-state engine slot (docs/01 §7.2). A scaffold on trunk.
pub const EXPLICIT: &str = "continuum-engine-explicit";
/// The partial-order reduction engine slot (RFC 0014). Not on trunk.
pub const DPOR: &str = "continuum-engine-dpor";

/// Who answered: the slot the engine fills and the build that filled it. Both are
/// pinned in every defect report (plan §4.7, "engine identity").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EngineIdentity {
    /// The engine slot, one of this module's constants or another stable name.
    pub slot: String,
    /// The build: crate name and version, or a mutant's label.
    pub build: String,
}

/// What one evaluation may spend, declared before it starts. No default: a default
/// budget is a truncation policy nobody reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Budget {
    /// Most states discovered.
    pub states: usize,
    /// Deepest breadth-first layer, for engines that search by layers.
    pub depth: usize,
    /// Most labelled transitions counted.
    pub transitions: u64,
}

/// Which normalized fields an engine answers, declared by the engine and not read off
/// its answer. The harness derives the set of invariants an engine must judge from
/// the **model** (every declared predicate) whenever `invariants` is set, so an engine
/// cannot opt out of coverage by returning nothing (cr-2r0m24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fields {
    /// The engine judges every predicate of the model as an invariant.
    pub invariants: bool,
    /// The engine reports a reachable-state projection.
    pub projection: bool,
}

impl Fields {
    /// Both fields: verdicts for every predicate, and a projection.
    pub const ALL: Self = Self {
        invariants: true,
        projection: true,
    };
    /// A projection and no verdicts (the tiny exhaustive oracle).
    pub const PROJECTION_ONLY: Self = Self {
        invariants: false,
        projection: true,
    };
}

/// An engine, as the harness sees it.
pub trait Engine {
    /// The slot and build.
    fn identity(&self) -> EngineIdentity;

    /// The fields this engine answers. No default: an adapter states its contract.
    fn fields(&self) -> Fields;

    /// Answer every question about `model` within `budget`, normalized.
    ///
    /// Total: an engine failure is a typed [`super::normal::Inconclusive`] with reason
    /// `EngineError`, a budget stop is `ResourceExhausted`, and semantics the engine
    /// does not handle is `Unsupported`. None of them is a verdict (INV-008). An
    /// `EngineError` is an engine fault, not a typed inconclusive: the harness always
    /// reports it through the defect lifecycle, whether one side or both sides of a
    /// lane report it. A panic here is caught by the harness and is always an engine
    /// defect; a panic in [`Engine::identity`] or [`Engine::fields`] is a contract
    /// fault of the slot the engine was plugged into, reported with a `defect_*`
    /// handle. Both are read once, at assembly. Every engine fault halts every lane its
    /// slot serves.
    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized;
}
