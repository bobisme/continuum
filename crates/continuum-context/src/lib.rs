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
//! [`guarantee`], [`causal`], [`stage`], [`compile`] (bn-21vno — the compiler's **stage
//! group 1**, RFC 0028 stages 1–2 with the redaction pre-pass) — the ten-stage pipeline
//! begins here. [`guarantee`] carries the closed thirteen-member `guarantees` vocabulary and
//! makes rule C1 a type: a request has no path into a claim, and a claim needs a
//! [`guarantee::License`] no code outside this crate can mint. [`causal`] is the
//! engine-independent causal order stage 2 slices over — `continuum-cir` is a stub and this
//! crate may not import an engine — together with the *independent* closure checker RFC 0028's
//! Validation section requires and a declared [`causal::Completeness`] that decides whether a
//! stage-2 drop is `slice-irrelevant` (a proof) or `heuristic-cutoff` (an undecided). [`stage`]
//! is the auditable-intermediate spine: all ten stages named, the redaction pre-pass ordered
//! before stage 1 as RFC 0028 correction 12 requires, and a per-phase retrievable record for
//! RFC 0030's compared artifact 5. [`compile`] runs the group — pre-pass, root selection,
//! backward causal slicing — publishing through [`accounting`] so INV-007's counting equation
//! holds by construction, and refusing a redacted root, a hole redaction punched in a core
//! (the guarantee is dropped, never the honesty), and a residual expansion query nothing
//! published anchors.
//!
//! [`property`], [`monitor`], [`dependence`] (bn-1kj2n — the compiler's **stage group 2**, RFC
//! 0028 stages 3–4) — the pipeline's next two stages, appended to the same trail and the same
//! accounting. [`property`] carries the finite, deterministic property automaton stage 3 is
//! directed by and the filter itself; the automaton declares its *relevance* set as the
//! alphabet **together with** the abstraction-relevant hidden events, because "a compiler MUST
//! NOT exclude an abstraction-relevant hidden event on the grounds that no observer publishes
//! it", and a declared [`property::Coverage`] decides — exactly as [`causal::Completeness`]
//! does for stage 2 — whether a stage-3 drop is a proof or an undecided. [`monitor`] is the
//! **independent** property monitor RFC 0028's Validation section requires ("an implementation
//! that reuses the compiler's own automaton has checked nothing"): a different computation in a
//! different module, which runs the automaton over the whole causal order and over the proposed
//! selection and compares the two runs, and whose verdict is the only thing that licenses
//! `PropertyPreserving`. [`dependence`] is stage 4, which licenses no guarantee at all but the
//! *admissibility* of `source` and `model` items: the untrusted static
//! [`dependence::SourceCorrespondence`] and the trusted dynamic
//! [`dependence::ExecutionDependence`] are two types with no conversion between them, so a
//! dependence claimed only by untrusted source input cannot silently upgrade into a selected
//! item (INV-016), and every stage-4 decline is `heuristic-cutoff` because an uncorroborated
//! claim is undecided rather than disproved.
//!
//! [`unsupported`], [`proof`], [`observer`], [`scope`], [`correspondence`] (bn-imhw2 — the
//! compiler's **stage group 3**, RFC 0028 stages 5–7) — the group whose load-bearing work is a
//! *refusal*. Two of its three stages have no producing subsystem in this workspace:
//! `continuum-proof-client` (RFC 0035's proof service, stage 5's slicer and the `ProofRelevant`
//! checker) and `continuum-refinement` (plan §16's correspondence graph, stage 7's input) are
//! PR-1 / IMPL-01 scaffolds with no public item at all. RFC 0026's
//! `rule errors.unsupported_surface` decides what a stage does about that — refuse, "rather
//! than degrading, guessing, or returning an empty success" — so [`unsupported`] carries the
//! typed refusal vocabulary those two stages record in, keeping "no proof service in this
//! deployment" and "the question named nothing to slice" apart as INV-008 requires;
//! [`proof`] is stage 5, whose guarantee `ProofRelevant` is additionally **unreachable by
//! construction** (no line of this crate issues that licence, and the integration suite holds
//! every `License::issue` call site to a recorded inventory); and [`correspondence`] is stage 7,
//! which decides no candidate kind and therefore deliberately invents no manifest cell. Both
//! refusals are pinned to the workspace state that justifies them by *freshness tripwires* that
//! go red the moment either producer grows a public surface.
//!
//! Stage 6 is the exception and is implemented for real, because its input is here: the intent's
//! observers are [`continuum_intent::observers::ObserverSet`], a landed field group in a crate
//! this one already depends on. [`observer`] carries stage 6's premise — the observer set
//! together with a declared attribution of candidates to projection elements — and the filter,
//! which keeps whatever the property protects and re-closes, so observer scoping structurally
//! *cannot* undo stage 3 and cannot break stage 2's closure ("a compiler MUST NOT exclude an
//! abstraction-relevant hidden event on the grounds that no observer publishes it"). A candidate
//! whose observability nobody declared is never dropped: INV-013 justifies a reduction relative
//! to named observers, and there is no justification for removing what nobody described.
//! [`scope`] is the **independent** audit of that reduction — a different computation in a
//! different module, over the same premise — whose affirmative verdict licenses *nothing*,
//! because RFC 0028's Licenses column for stage 6 reads "never a guarantee by itself".
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
pub mod causal;
pub mod compile;
pub mod correspondence;
pub mod dependence;
pub mod event;
pub mod expansion;
pub mod guarantee;
pub mod model;
pub mod monitor;
pub mod observer;
pub mod omission;
pub mod pack;
pub mod proof;
pub mod property;
pub mod replay;
pub mod scope;
pub mod selection;
pub mod source;
pub mod stage;
pub mod state_delta;
pub mod target;
pub mod unsupported;
pub mod verdict;
