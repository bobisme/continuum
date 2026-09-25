//! `continuum-incremental` — the incremental semantic database (plan §9, PR 23).
//!
//! # Responsibility
//!
//! Content-addressed queries for parsing, model construction, property automata,
//! exploration, context, and diff, with dependency edges classified as exact,
//! validated, conservative, or experimental.
//!
//! The build-vs-adopt decision for the engine itself is an ADR (PR 22a) that gates
//! Phase C.
//!
//! # What this crate delivers today: the PR-22A / IMPL-02 spike (bn-31vf)
//!
//! Decision: RFC 0030. A *custom* engine — no memoization framework — over the CML
//! front end and the reference engine, built to give the build-vs-adopt ADR measured
//! evidence rather than a preference:
//!
//! - [`vocab`]: RFC 0030's closed sets — the four reuse-edge classes with their meet,
//!   the nine dependency reasons, the three auditability classes, the five evidence
//!   forms, the independence tri-state, and the two audit lanes. Unknown tokens fail
//!   closed.
//! - [`query`]: the registry of query definitions (`parse`, `elaborate`,
//!   `build_model`, `project_transition_system`, `project_invariant`, `explore`,
//!   `domain_safety`, `check_invariant`), each edge with its reason, class and
//!   recorded assumption, and the canonical [`query::QueryKey`].
//! - [`engine`]: the memoizing engine. Every reuse is licensed by one class — `Exact`
//!   by content identity, `Conservative` through a side input under a recorded
//!   assumption, `Validated` by a finite-closure certificate the independent kernel
//!   verifies for the current model, `Experimental` by a deliberately unsound textual
//!   heuristic in the interactive lane only — and then checked against the
//!   quarantine set and the lane.
//! - [`explain`]: `query.explain_invalidation` over two revisions: every query lands
//!   in exactly one of `invalidated`, `unknown`, `reused`, and each invalidation
//!   carries the chain of edges, reasons and classes from the changed input.
//! - [`audit`]: the Incremental Parity Audit's clean side (INV-010). It recomputes
//!   every compared artifact from the source with no cache, no key, no class and no
//!   projection, and it shares no decision logic with [`engine`] — only the canonical
//!   encoders of [`output`], which define what is compared.
//!
//! # Where the spike departs from RFC 0030, on purpose
//!
//! - The `Experimental` heuristic reuses across an edit whose independence is
//!   heuristic, which RFC 0030 downgrade rule 4 forbids for every class. The spike
//!   confines it to the interactive lane, never reports it as `reused`, and lets the
//!   audit quarantine it; whether previews may do this at all is for the ADR.
//! - The promotion lane here only refuses `Experimental` reuse. RFC 0030's promotion
//!   lane also recomputes or independently validates every promotion-relevant query.
//! - `explore` keys its bounds as `strategy_config` and is declared
//!   equality-auditable: the reference engine is deterministic under fixed bounds,
//!   and the bounds are the declared model-checking scope. RFC 0030's budget rule
//!   (budget out of the key, frontier inclusion for budget-truncated searches) is
//!   not modelled.
//! - A `Validated` witness is minted speculatively for the new input, so one the
//!   kernel rejects is a refused licence, not the defect RFC 0030 names for a
//!   stored witness that fails to re-check.
//! - A recompute after a downgrade overwrites the entry under its key; there is no
//!   `SUPERSEDES` edge. Quarantining a triple evicts every entry whose provenance
//!   holds it, so nothing produced under it is served again. The key pins only the semantic epoch; the other five are
//!   unpinned by omission rather than as named `Unpinned` entries.
//!
//! - The differential test's oracle, `continuum-engine-reference`'s own checker
//!   (`tests/differential.rs`), checks definedness since bn-24a5c, and its typed
//!   `Undefined` outcome is compared with this engine's `Undefined` and
//!   `UndefinedAction` on partial-map models, including nested chains
//!   (`I#defined#defined`), gaps, and action-name collisions (bn-1eoco). Both sides
//!   classify a model's definedness predicates with the same rule,
//!   `continuum_model_core::definedness::Definedness::of` — the engine
//!   ([`engine`]) and the independent audit ([`audit`]) each call it directly,
//!   rather than re-deriving the chain rule, so a nested chain, a gap, or a
//!   collision reads the same way on every path that classifies one.
//!
//! What is not here: persistence and crash ordering, the index verifier, epoch
//! `Revalidate` demotion, witness-loss downgrade, quarantine clearing, continuations,
//! frontier inclusion for budget-sensitive definitions, and the Lean computed-cone
//! theorem. The daemon's `query.explain_invalidation` stays unwired: it is keyed by
//! a `diff_*` handle naming an RFC 0031 artifact, and the daemon cannot build one yet
//! (`crates/continuumd/src/daemon/workspace.rs`, `workspace.fork`'s `pre_diff`).
//! The spike builds no wire projection: [`explain`] gives one explanatory chain
//! per invalidation, not the complete, sorted `<handle>:<reason>` edge set that
//! `rule query.invalidation_edges` requires. That projection is an ADR residual.
//!
//! # Dependency-boundary contract
//!
//! - INV-010 — incremental parity: the incremental engine is audited by an independent
//!   clean-result comparison (Incremental Parity Audit, plan §9.5), which may not share
//!   decision logic with it.
//! - May not import adapters or `continuum-forge`.
//! - A `Validated` witness is checked only by `continuum-certificate` (the kernel,
//!   from wire form), never by the engine that produced it (INV-004).
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

#![forbid(unsafe_code)]

pub mod audit;
pub mod engine;
pub mod explain;
pub mod output;
pub mod query;
pub mod vocab;
