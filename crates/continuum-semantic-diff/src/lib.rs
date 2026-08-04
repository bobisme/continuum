//! `continuum-semantic-diff` — semantic and intent diff (plan §5.3, docs/40, PR 12).
//!
//! # Responsibility
//!
//! Classification of changes — exact equality, property AST edit, assumption
//! add/remove, bound change, observer event change, fault/fairness/assurance change —
//! and the impact analysis derived from them.
//!
//! Solver-based implication is admitted only where it is sound and bounded; everything
//! else is reported as a privileged change.
//!
//! # Dependency-boundary contract
//!
//! - Diffs describe change; they never apply it. Application belongs to `continuum-
//!   repair`.
//! - May not import adapters or `continuum-forge`.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically; this
//! crate's one declared edge, `continuum-intent`, is downward (the field-group types
//! and the closed classification vocabulary a diff classifies from and into) and is
//! not forbidden by any of its rules.
//!
//! # What has landed, and what it is for
//!
//! PR 12 names six classification bullets (`notes/START_HERE_IMPLEMENTATION.md`):
//! exact equality, property AST edit, assumption add/remove, bound change, observer
//! event change, and fault/fairness/assurance change. PR-12 / IMPL-05 (`bn-1sdp`)
//! delivers exactly the fifth:
//!
//! - [`observers`] — RFC 0031's `observers` field classification: componentwise
//!   comparison of an observer's four sets (`events`, `state_projection`,
//!   `knowledge_projection`, `security_projection`) into `unchanged` / `refined` /
//!   `coarsened`, plus `added`/`removed` membership for units present on one side
//!   only. This is the mechanism behind the "coarsen observer" intent attack
//!   (`docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`) and the "hide observer
//!   events" attack RFC 0031 names directly (plan §19.5): dropping an event family or
//!   a projection element is what the module's doc calls the load-bearing case, and
//!   its tests include a dedicated "hiding is never benign" adversarial suite — a
//!   same-size event swap, and a coarsen-then-rename compound — none of which the
//!   classifier can read as `unchanged`, `refined`, or a clean rename.
//!
//! The other five bullets are separate open bones — `bn-b8ru` (IMPL-01, exact
//! equality), `bn-8mlg` (IMPL-02, property AST edit), `bn-7vg7` (IMPL-03, assumption
//! add/remove), `bn-ycn6` (IMPL-04, bound change), `bn-3vxp` (IMPL-06,
//! fault/fairness/assurance change) — and land as their own reviewable units. Also
//! not here: the wire `intent_changes[]`/`diff_*` artifact assembly across all fifteen
//! protected fields (`schemas/semantic-diff.schema.json`), and the impact-set
//! computation (RFC 0031, "Impact set"). [`observers`]'s module doc states its own
//! narrower boundary in full under "Scope: PR-12 / IMPL-05 only".
//!
//! PR-12 / IMPL-04 (`bn-ycn6`) adds [`bounds`] — RFC 0031's `bounds` field classification, the componentwise `(values, nodes, faults, depth)` order into `unchanged`/`expanded`/`contracted`/`incomparable`/`unknown`; see its module doc for scope.
//!
//! PR-12 / IMPL-06 (`bn-3vxp`) adds [`faults`], [`fairness`], and [`assurance`] — RFC 0031's `faults` (fault-model set membership), `fairness` (per-unit membership, the `kind` movement, and the antitone `condition` rule), and `assurance` (the requirement triple's total order, closed over `continuum_intent::assurance_policy` and never emitting `incomparable`) field classifications; see each module's doc for scope.
//!
//! PR-12 / IMPL-03 (`bn-7vg7`) adds [`assumptions`] — RFC 0031's `assumptions` field classification: per-unit `added`/`removed` membership keyed by `id`, `unchanged`/`incomparable` for declaredness moves on `classification`/`fidelity_profile` with the expression held fixed, and `unknown` for an unequal expression (no Finite-fragment inclusion oracle exists yet to discharge `strengthened`/`weakened`); the dangerous direction is `strengthened`, the mirror image of `properties`' `weakened` — see the module doc for scope and the recorded RFC 0031 table gap.
//!
//! PR-12 / IMPL-01 (`bn-b8ru`) adds [`equality`] — RFC 0031's whole-contract `unchanged` shortcut: `classify` decides whether two Intent Contract identities agree, licensing all fifteen fields `unchanged` without inspecting any of them; see its module doc for scope and for why it answers plan §5.3's "unchanged intent", the bullet's normative text rather than its two-word title.

pub mod assumptions;
pub mod assurance;
pub mod bounds;
pub mod equality;
pub mod fairness;
pub mod faults;
pub mod observers;
