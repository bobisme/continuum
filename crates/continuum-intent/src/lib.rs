//! `continuum-intent` — the protected Intent Contract (plan §5, PR 4).
//!
//! # Responsibility
//!
//! Schema and types for properties, assumptions, observers, bounds, faults, fairness,
//! assurance policy, optimization/non-vacuity, field-level change policy, and intent
//! locks.
//!
//! Intent is the question under verification. INV-001: ordinary operations cannot
//! mutate it; a change to intent is a privileged, diffed, and recorded event.
//!
//! # What has landed, and what it is for
//!
//! PR-4 / IMPL-01 delivers the **`properties` field group** — RFC 0037's `claims`,
//! their CPNF-1 property expressions, and their canonical identities:
//!
//! - [`canonical_json`] — RFC 0037 ID5's byte spelling. One canonical JSON writer
//!   and one strict reader for the whole crate; the other fourteen field groups
//!   reuse them rather than opening a second encoding.
//! - [`ast`] — the Finite-core property fragment: eleven formula node kinds, five
//!   term node kinds, `next` absent by construction.
//! - [`cpnf`] — CPNF-1 normalization, rules N1–N9, with every declined rewrite
//!   recorded and cited.
//! - [`property`] — [`property::Claim`], [`property::ClaimSet`], and
//!   [`property::PropertyIdentity`]: a property's unit key, its class, its
//!   normalized formula, and the identity derived from its canonical bytes.
//!
//! Two `pub(crate)` modules carry what all the field groups share, so that each rule
//! is stated once rather than once per group:
//!
//! - `codec` — the crate's one reader for `$defs/property_expression` and the
//!   `$defs/formula`/`$defs/term` node set beneath it. It is entered by
//!   [`ast::Formula::from_json`] (a bare formula, as `fairness[].condition` carries)
//!   and [`property::PropertyExpression::from_json`] (the expression object, as
//!   `claims[]` and `assumptions[]` carry). Two readers for one grammar is how a
//!   document comes to have two meanings.
//! - `identity` — the ADR-0013 discipline, once. Each group's public identity type is
//!   a distinct newtype over the one shared core, so a checker cannot pass an
//!   observer's identity where a claim's is expected, and no group can quietly become
//!   the one whose identity is a digest.
//!
//! ## Why this group first
//!
//! The Intent Contract exists to make a *weakening* visible, and RFC 0031 is allowed
//! to call a property change `unchanged` on exactly one thing:
//!
//! > **R2 — `unchanged` on equality and nothing weaker.** `unchanged` MUST be
//! > claimed only on equality of the canonical encoding of the classified unit: the
//! > CPNF-1 N8 encoding for units carrying a property AST […] Textual match, partial
//! > structural match, and label/comment equality MUST NOT be used.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`
//!
//! Everything else in the contract is compared by set membership or by a declared
//! order. The property group is the only one whose comparison needs a normal form,
//! and `docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md` lists "weaken property"
//! first among the intent attacks. So the normal form is the load-bearing piece, and
//! it is the piece the sibling groups depend on: `assumptions[].expression` is the
//! same [`property::PropertyExpression`], and `fairness[].condition` is a bare
//! temporal-free [`ast::Formula`].
//!
//! ## Immutability, and what "a change" means here
//!
//! No type in this crate has a `&mut self` method. RFC 0037: "Accepted contracts are
//! immutable. A revision never edits a stored contract: it mints a new `in_*` and
//! records a supersession edge. Nothing here permits an in-place edit of an accepted
//! contract, including a 'cosmetic' one." A changed claim is therefore a new
//! [`property::Claim`] with a new [`property::PropertyIdentity`], and the API offers
//! no other spelling — not because mutation would be *inconvenient*, but because a
//! setter is the affordance by which an ordinary operation mutates intent, which is
//! the single thing INV-001 forbids.
//!
//! ## Still to land in PR 4
//!
//! `assumptions`, `observers`, `bounds`, `faults`, `fairness`, the assurance policy,
//! `optimization`/non-vacuity, and the field-level change policy are separate bones.
//! Each module below names the seams it leaves them; in summary: they encode through
//! [`canonical_json`], the two that carry property ASTs reuse [`ast`] and [`cpnf`]
//! unchanged, and the contract-level identity (`in_*`, ID1) composes their preimages
//! with [`property::ClaimSet::identity`].
//!
//! # Dependency-boundary contract
//!
//! - Top of the dependency islands, beside `continuum-workspace`.
//! - The Intent Contract parser and policy sit inside the smallest trust base (docs/33
//!   "Trust boundary"), so this crate may not import engines, adapters, `continuum-
//!   forge`, or `continuum-asupersync`.
//! - One workspace edge: `continuum-value`, the only declared leaf crate, for the
//!   ADR-0013 identity discipline and its `ContentHasher`/`Digest256` seam. Identity
//!   is canonical bytes; a digest indexes; a collision resolves by canonical
//!   comparison. This crate applies that discipline to RFC 0037's byte spelling
//!   rather than restating it, and makes no hash-vendor decision of its own.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod assumptions;
pub mod assurance_policy;
pub mod ast;
pub mod bounds;
pub mod canonical_json;
pub mod change_policy;
pub(crate) mod codec;
pub mod cpnf;
pub mod fairness;
pub mod faults;
pub(crate) mod identity;
pub mod observers;
pub mod optimization;
pub mod property;
