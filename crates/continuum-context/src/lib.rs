//! `continuum-context` — Context Pack schema and compiler (plan §6, PR 11).
//!
//! # Responsibility
//!
//! Target, verdict, and assurance; state and event slices; source and model references;
//! omissions and expansion handles; replay reference; byte and token budgets.
//!
//! Context is compiled against a budget, not dumped. The compiler's output is audited
//! by independent replay and property-preservation checks (docs/33 "Architectural
//! separation").
//!
//! # Dependency-boundary contract
//!
//! - INV-007 — omission transparency: everything dropped is named and expandable.
//! - INV-013 — property-scoped reduction: reduction is justified relative to the
//!   property under check.
//! - Sits below the verification island and above the adapters; may not import adapters
//!   or `continuum-forge`.
//! - Depends on `continuum-value` (the `Name` identifier grammar), `continuum-workspace`
//!   (`WorkspacePath`, `ArtifactHandle`/`ArtifactClass`) and `continuum-intent`
//!   (`canonical_json::Json`, this workspace's one canonical-JSON writer, already reused
//!   cross-crate by `continuum-benchmark`) — all three sit above this crate in the
//!   dependency-islands diagram (`START_HERE_IMPLEMENTATION.md`, "Dependency islands":
//!   `continuum-workspace / continuum-intent` before `context`), so the edges add no new
//!   layering violation and no second encoding.
//!
//! # What is here
//!
//! [`selection`] (PR-11 / IMPL-03) — the pack's shared `selected[]` item shape
//! (`SelectedItem`) and the closed eleven-member `kind` vocabulary (`SelectionKind`)
//! every field group's items are drawn from.
//!
//! [`source`] (PR-11 / IMPL-03) — `selected[].kind = "source"`: [`source::SourceRef`], a
//! typed reference to a source span, never to interpolated source text (INV-016).
//!
//! [`model`] (PR-11 / IMPL-03) — `selected[].kind = "model"`: [`model::ModelActionRef`], a
//! typed reference to a named model action, optionally inside a real `model_*` elaborated
//! model artifact.
//!
//! Declined here, and left to their own bones: typed construction for the other nine
//! `SelectionKind` members (`event`, `state_delta`, `proof`, `assumption`,
//! `counterfactual`, `obligation_flow`, `order_constraint`, `repair_surface`, `unknown`);
//! the expansion protocol (`ExpansionRelation`, `expansions[]`, the omission manifest —
//! IMPL-04); target/verdict/assurance, the replay reference, and byte/token budgets
//! (IMPL-01, IMPL-05, IMPL-06); and whole-pack assembly, identity, and JSON-schema
//! validation (no IMPL bullet is that seam by itself; see `selection`'s module
//! documentation).
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod model;
pub mod selection;
pub mod source;
