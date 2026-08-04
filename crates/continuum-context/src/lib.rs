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
//! [`target`], [`verdict`], [`assurance`] (PR-11 / IMPL-01) — the pack's answer header:
//! the class-checked `in_*` contract and the typed `question` a pack was compiled for, the
//! closed four-member `verdict` whose `inconclusive` arm carries its INV-008 reason so an
//! untyped one has no spelling, and the `{class, envelope}` block reusing
//! `continuum_value::assurance`'s RFC 0031-ordered class and its nine-dimension B11
//! envelope.
//!
//! [`event`] and [`state_delta`] (PR-11 / IMPL-02) — `selected[].kind` in `{event,
//! state_delta}`: [`event::EventRef`], a typed reference to a causal-core event,
//! optionally inside a real `cir_*` causal execution graph, and
//! [`state_delta::StateDeltaRef`], a typed per-variable concrete-or-abstract value
//! transition, never a whole state.
//!
//! [`omission`] (PR-11 / IMPL-04) — the INV-007 manifest: the closed five-member reason
//! vocabulary, and a record shape in which the schema's two conditionals on `expandable`
//! are unrepresentable rather than validated.
//!
//! [`expansion`] (PR-11 / IMPL-04) — the expansion protocol: the closed eleven-member
//! `ExpansionRelation`, the `{relation, anchor}` query an omission points at, the depth it
//! is asked to, the **content-derived** [`expansion::ExpansionHandle`] that query resolves
//! to, and the payload pairing a record with the items it accounts for.
//!
//! [`accounting`] (PR-11 / IMPL-04) — closed accounting: a compile's selection and its
//! manifest are read off one ledger over the candidate set, so "candidate set = selection +
//! Σ manifest counts" holds by construction and an undispositioned candidate cannot be
//! published at all.
//!
//! [`pack`] (PR-11 / IMPL-04, measured by IMPL-06) — the expansion *child document*: a
//! parent pack's own bytes with the nine keys an expansion decides replaced and the rest
//! inherited verbatim, and `content_budget.bytes` carrying the child's own measured
//! canonical size (RFC 0028 correction 17). It derives; it does not compile.
//!
//! [`replay`] (PR-11 / IMPL-05) — the pack's top-level `replay` field, not a
//! `selected[].kind` member: [`replay::ReplayRef`], a typed, class-checked reference to a
//! `crash_*` crashpack.
//!
//! [`budget`] (PR-11 / IMPL-06) — byte budgets: RFC 0028's second branch for a ceiling the
//! answer exceeds. A smaller child is packed — a prefix of the declared item order, the
//! manifest reserved before any of it — and every candidate the ceiling dropped is recorded
//! under `budget` with the query that retrieves it, through the same closed accounting the
//! manifest is derived from. Below the smallest conforming child the branch flips: nothing
//! is published and the caller is refused.
//!
//! Declined here, and left to their own bones: typed construction for the remaining
//! seven `SelectionKind` members (`proof`, `assumption`, `counterfactual`,
//! `obligation_flow`, `order_constraint`, `repair_surface`, `unknown`) —
//! IMPL-04 carries items of any kind through an expansion and constructs items of none,
//! because `SelectedItem`'s four typed constructors are IMPL-03's (`source`, `model`) and
//! IMPL-02's (`event`, `state_delta`), and no PR-11 bullet owns the other seven — they
//! remain wire tokens with no reference type, carried through expansions unclaimed
//! (IMPL-01 and IMPL-05 turned out to own the answer header and the top-level `replay`
//! field, not kinds); a **token**
//! count, which is advisory, model-relative, and owed a tokenizer identity this workspace
//! does not have, so `content_budget.tokens` is absent rather than invented (`pack`'s module
//! documentation); a utility **ranker** for stage 9, whose absence is why `budget` packs the
//! declared canonical order's prefix and says so; and whole-pack *compilation* — the
//! ten-stage pipeline that turns evidence into a first pack — which no IMPL bullet is by
//! itself (see `selection`'s module documentation) and which `pack`'s own documentation is
//! careful not to claim.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod accounting;
pub mod assurance;
pub mod budget;
pub mod event;
pub mod expansion;
pub mod model;
pub mod omission;
pub mod pack;
pub mod replay;
pub mod selection;
pub mod source;
pub mod state_delta;
pub mod target;
pub mod verdict;
