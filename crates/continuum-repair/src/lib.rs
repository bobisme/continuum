//! `continuum-repair` — repair transactions (plan §8, PR 20).
//!
//! # Responsibility
//!
//! begin / apply / evaluate / promote over a hypothesis, a patch identity, exact
//! replay, semantic and intent diff, accumulated evidence, and a policy verdict.
//!
//! A repair is a transaction with gates, not an edit: a property-weakening patch is
//! reclassified and blocked rather than merged.
//!
//! # Dependency-boundary contract
//!
//! - INV-011 — intent-preserving repair.
//! - May not import adapters or `continuum-forge`; Forge consumes repair and verifier
//!   interfaces, never the reverse.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What is here: PR-20 / IMPL-01, the hypothesis (bn-2d70)
//!
//! The normative sources are `notes/plan/schemas/repair-transaction.schema.json` (the
//! artifact shape, INV-003) and RFC 0032 (the obligations the schema cannot state).
//! Both fix the hypothesis narrowly, and this crate follows them:
//!
//! > | `hypothesis` | string | yes | untrusted prose; never evidence (see below) |
//! >
//! > **The hypothesis is not evidence.** `hypothesis` MUST NOT contribute to any gate
//! > outcome, to the policy verdict, or to the receipt's decision, and MUST NOT be
//! > interpolated into any typed field or into error text (INV-003, INV-016). It is
//! > rendered to reviewers and parsed by nothing.
//! >
//! > — RFC 0032, "The transaction object"
//!
//! So [`hypothesis::Hypothesis`] is an opaque string that nothing parses. What a repair
//! *targets* is typed, and it is typed by the transaction, not by the prose: the frozen
//! base triple (`failure: crash_*`, `base_snapshot: ws_*`, `base_intent: in_*`). And
//! what a repair may *not* do — change protected intent — is refused on the typed
//! `changes`, where RFC 0032 puts the refusal: a change of kind `intent` through
//! ordinary repair authority is `IntentMutationDenied` at `repair.apply`.
//!
//! | Module | Content |
//! |---|---|
//! | [`handle`] | the `crash_`, `ws_` and `rt_` handle patterns of the schema |
//! | [`hypothesis`] | the untrusted hypothesis, the closed change kinds, the proposal |
//! | [`transaction`] | the transaction skeleton: `begin` (draft v1) and `apply` (applied v2+), canonical bytes, content identity |
//! | [`patch`] | patch identity (PR-20 / IMPL-02, bn-195b): declared changes, the candidate sealed from base + changes, gate 2's comparison |
//! | [`receipt`] | the promotion-receipt skeleton (PR 22): intent identity, before/after snapshots, `gate_profile` and the `NotYetEnforced` list, composed from referenced artifacts and verified against them |
//!
//! # What is here: PR-22 / IMPL-01, IMPL-02, IMPL-07 and IMPL-08, the receipt skeleton (bn-1ebx, bn-cps6, bn-1cec)
//!
//! RFC 0032 is the normative home of the receipt's composition, and the receipt cites a
//! repair transaction, so the receipt lives beside the transaction. [`receipt`] derives
//! `intent`, `base_snapshot`, `result_snapshot`, `gate_profile` and the
//! `not_yet_enforced` entries of `gates` from a [`transaction::RepairTransaction`] and
//! three daemon stores, and verifies a client-supplied receipt's same fields by
//! re-deriving them. Every other receipt field is a typed [`receipt::ReceiptSeam`] with
//! its owner, and [`receipt::SkeletonVerification::receipt_verdict`] is never a full
//! verification.
//!
//! # Seams left for the later PR-20 bones
//!
//! - **IMPL-02, patch identity, is here** ([`patch`]). `apply` takes the base content
//!   and the declared file edits, seals the candidate from them through
//!   `continuum-workspace`'s snapshot, and derives every `changes[].digest`. No caller
//!   names a candidate. What is left for later bones: gate 2 as a recorded gate status
//!   (IMPL-03), and a durable store for the declared change payloads the digests name,
//!   so that gate 2 can re-seal a published version (the daemon's).
//! - **IMPL-03, exact replay; IMPL-05, evidence accumulation.** Every gate is `pending`
//!   or `not_yet_enforced` with empty evidence. No operation here writes a gate
//!   outcome or spends from the cost ledger.
//! - **IMPL-04, semantic and intent diff.** A `model` or `rust` change that weakens a
//!   property is detected only by the RFC 0031 diff behind gate 3. `semantic_diff` is
//!   never emitted here.
//! - **IMPL-06, policy verdict.** `policy_verdict` and `receipt` are never emitted,
//!   and the status derivation is only the two rules the skeleton can reach
//!   (`draft` iff no candidate snapshot, else `applied` because no gate has run).
//! - **Daemon wiring.** `repair.begin` and `repair.apply` are declared by the IDL but
//!   not served; the daemon owns crashpack resolution ([`transaction::FailureBinding`]),
//!   lineage heads (an `apply` on a non-head version is a lost compare-and-set),
//!   idempotency, publication, and the request-size bound on the hypothesis and the
//!   change list, which the schema leaves unbounded. The mapping of
//!   [`transaction::BeginRefusal`] and [`transaction::ApplyRefusal::VersionExhausted`]
//!   onto the IDL's closed error sets is the daemon's too: `repair.begin` declares only
//!   `UnsupportedSemanticFeature` and `PolicyGateFailed` beyond `rule errors.common`,
//!   and none of those names an unresolved crashpack. `BindingUnavailable` must keep a
//!   code distinct from "unknown" on the wire (INV-008).
//!
//! # Open questions raised, not resolved here
//!
//! - **A patch that does not apply is refused at `apply`, with no named wire code.**
//!   RFC 0032's `apply` "seals a candidate snapshot from base + changes", and a change
//!   that does not apply seals nothing, so [`patch::PatchRefusal`] is an `apply`
//!   refusal. Gate 2 keeps its comparison role ([`patch::SealedCandidate::check`]) for
//!   a candidate built elsewhere. But the IDL's `repair.apply` union is only
//!   `IntentMutationDenied` and `UnsupportedSemanticFeature` plus `rule errors.common`,
//!   the RFC does not state the path-disjointness rule, and `BaseMismatch` is a store
//!   defect rather than a caller error. The RFC needs a correction naming the codes; the
//!   mapping is the daemon's (bn-23pwm).
//! - **Two `begin` calls on one crashpack mint one `rt_`.** RFC 0032 says both that
//!   `repair_id` is the content identity of the version and that "two distinct `begin`
//!   calls against one crashpack are two transactions". A v1 preimage is fully
//!   determined by the failure, its binding and the profile, and the schema
//!   (`additionalProperties: false`) has no field that could carry a per-transaction
//!   discriminator; `actor` is provenance only. The two clauses cannot both hold today.
//!   This crate follows the first and does not invent a field; the RFC or the schema
//!   must add the discriminator.
//! - **`status` is in the identity preimage.** Every schema field but `repair_id` is.
//!   When a later bone derives `superseded` for a non-head version, a published version
//!   would need new bytes, which "nothing is rewritten" forbids. IMPL-06 must decide
//!   whether `superseded` is a projection kept outside the published artifact.

pub mod handle;
pub mod hypothesis;
pub mod patch;
pub mod receipt;
pub mod transaction;
