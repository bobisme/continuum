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
//! fault/fairness/assurance change) — and land as their own reviewable units.
//! [`observers`]'s module doc states its own narrower boundary in full under
//! "Scope: PR-12 / IMPL-05 only".
//!
//! PR-12 / IMPL-04 (`bn-ycn6`) adds [`bounds`] — RFC 0031's `bounds` field classification, the componentwise `(values, nodes, faults, depth)` order into `unchanged`/`expanded`/`contracted`/`incomparable`/`unknown`; see its module doc for scope.
//!
//! PR-12 / IMPL-06 (`bn-3vxp`) adds [`faults`], [`fairness`], and [`assurance`] — RFC 0031's `faults` (fault-model set membership), `fairness` (per-unit membership, the `kind` movement, and the antitone `condition` rule), and `assurance` (the requirement triple's total order, closed over `continuum_intent::assurance_policy` and never emitting `incomparable`) field classifications; see each module's doc for scope.
//!
//! PR-12 / IMPL-03 (`bn-7vg7`) adds [`assumptions`] — RFC 0031's `assumptions` field classification: per-unit `added`/`removed` membership keyed by `id`, `unchanged`/`incomparable` for declaredness moves on `classification`/`fidelity_profile` with the expression held fixed, and — since `bn-2nwpg` wired the content axis to `bn-8mlg`'s oracle through the shared `Finite` license arm — an affirmed `strengthened`/`weakened` for the single-axis expression edits the oracle decides, its relation consumed directly with **no dualize** (RFC 0031: "exactly as for claims — the same order"; the module doc derives the polarity against fairness's antitone contrast), with everything unlicensed, undecidable, budget-exhausted, or compounded with a declaredness move still failing closed to `unknown`; the dangerous direction is `strengthened`, the mirror image of `properties`' `weakened` — see the module doc for scope and the recorded RFC 0031 table gap.
//!
//! PR-12 / IMPL-01 (`bn-b8ru`) adds [`equality`] — RFC 0031's whole-contract `unchanged` shortcut: `classify` decides whether two Intent Contract identities agree, licensing all fifteen fields `unchanged` without inspecting any of them; see its module doc for scope and for why it answers plan §5.3's "unchanged intent", the bullet's normative text rather than its two-word title.
//!
//! PR-12 (`bn-7ek41`) adds the two halves that close semantic diff v0 over the
//! family: [`artifact`] — the wire `intent_changes[]`/`diff_*` artifact assembler
//! across all fifteen protected fields (`schemas/semantic-diff.schema.json`; the
//! equality shortcut's correction-14 empty wire array, the family's records consumed
//! with no second classification authority, the R2/fail-closed per-unit closure for
//! the eight classifier-less fields, the assembler-owned `unsupported` override and
//! `requested_assurance` clamp, and the P1-complete `PolicyTable::verdict` closure
//! rendered as the artifact's `policy` section) — and [`impact`] — RFC 0031's
//! "Impact set": the `invalidated`/`reused`/`unknown` partition over the evidence
//! naming either intent identity or either snapshot, computed against RFC 0030's
//! dependency reasons, reuse-edge classes, and the `Unknown`-is-dependent tri-state,
//! with the query-key rule for `properties` and the no-reason blanket-`unknown` rule
//! for the seven unmapped fields. See each module's doc for its authorities and
//! recorded divergences.
//!
//! PR-12 / IMPL-02 (`bn-8mlg`) adds [`properties`] — RFC 0031's `properties` field classification over `continuum-intent`'s `Claim`/`ClaimSet`, keyed by `id`: membership (`added`/`removed`, never matched by expression to defeat a rename), the meaning axis (`kind`/`observer` moved with the expression fixed is `incomparable`; moved with it, `unknown`, and the oracle is never consulted across a meaning change), and the content axis — `unchanged` on equal ID2 canonical encodings and, under the `Finite` fragment license alone, a **sound and bounded** implication oracle (a closed set of meaning-preserving derivation rules under a fixed step budget, START_HERE's "solver-based implication only where sound and bounded") deciding `strengthened`/`weakened`, failing closed to `unknown` wherever it cannot derive an inclusion and never emitting `incomparable` from content, since a failed derivation refutes nothing. The module also becomes the crate's one formula-comparison authority (`formula_relation`/`finite_formula_relation`), which [`fairness`] delegates its declared-condition arm to and [`assumptions`] consumes through the shared license arm (`bn-2nwpg`); see its module doc for the fragment license, the declined rules, and the both-directions collision arm.

pub mod artifact;
pub mod assumptions;
pub mod assurance;
pub mod bounds;
pub mod equality;
pub mod fairness;
pub mod faults;
pub mod impact;
pub mod observers;
pub mod properties;
