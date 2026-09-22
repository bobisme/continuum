//! The `context` family: `compile` and `expand` — the wire end of PR-11's Context Pack.
//!
//! # What lands here, and what does not
//!
//! **Both operations are served (bn-1y4qc).** `context.compile` runs `continuum-context`'s
//! landed pipeline — the redaction pre-pass and RFC 0028 stages 1–8, with stage 9 disabled
//! (the register row's own named fallback configuration) and stage 10 supplied by
//! [`continuum_context::budget`] — and publishes the root document
//! [`continuum_context::pack::RootPack`] writes. Until this bone the operation was refused
//! `UnsupportedSemanticFeature` and this module said why: "RFC 0028's compiler is ten stages
//! over the evidence graph, the property automaton, the correspondence graph and a minimizer,
//! and none of those exist in this workspace." Six of those stages now exist, two of them
//! refuse *inside* the pipeline with typed records rather than from the wire, and the
//! difference between an unbuilt compiler and a compiler whose stage 5 has no proof service is
//! precisely the difference `rule errors.unsupported_surface` draws between refusing an
//! operation and answering it honestly.
//!
//! # What a compile still needs from out of band, and why that is not a degradation
//!
//! Stage 2's declared input is "CIR causal order" and `continuum-cir` is a PR-17 scaffold, so
//! nothing in this workspace turns an `ev_*` evidence root into a candidate order. This module
//! does not invent one. A deployment registers the projection it holds —
//! [`ContextCompileSource`] — through
//! [`DaemonState::put_compile_source`](super::state::DaemonState::put_compile_source), the same
//! out-of-band administration surface content staging, capability provisioning and pack
//! registration already use ("content staging and capability provisioning are not operations in
//! this protocol version (IDL §7)"). An `evidence_root` no source is registered for is refused
//! `UnsupportedSemanticFeature`, uniformly — the same answer whether or not the daemon holds
//! that root, so the refusal is not an existence oracle (RFC 0027 X2).
//!
//! What this buys is exactly what the exit owed: the *compiler* is the producer. The candidate
//! order, the item bodies and the answer header are a deployment's registered facts; the
//! selection, the guarantees, the manifest, the accounting and the document are the landed
//! pipeline's.
//!
//! # `Audience` selects rendering, never content
//!
//! > Two `context.compile` requests differing only in `audience` MUST produce the same `ctx_*`.
//! >
//! > — RFC 0028, "Views and rendering"
//!
//! Held structurally rather than remembered: [`continuum_context::pack::RootIdentity::derive`]
//! takes the question, the intent, the snapshot, the semantic epoch and the evidence roots, and
//! `audience` is not among them and has no path to become one. This daemon renders one
//! projection — the pack itself — so the field selects between renderings it does not have and
//! changes nothing; a deployment that adds renderings adds them beside the artifact, never
//! inside it.
//!
//! Expansion is still a different operation, and the reason it landed first stands: it is
//! **navigation over a published pack**, not compilation from evidence.
//!
//! > `context.expand(context, anchor, relation, depth?)` returns a new immutable pack
//! > referencing its parent. Everything beyond the default result is reachable by
//! > expansion, never by dumping. […] `anchor` MUST resolve in the parent: it is either a
//! > `selected[].id` or an anchor named by one of the parent's own `expansions[]` or
//! > `omissions[].expansion` entries. Expansion is navigation over a published pack, not a
//! > general graph query.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! Every input of that operation is a published artifact plus a query over it, so this
//! family's answer is derived from a pack a deployment registered and from nothing else.
//! `continuum-context` owns the derivation ([`ChildPack`], the manifest, the accounting);
//! this module owns the wire mapping and the typed outcome table.
//!
//! # Where a pack comes from, honestly
//!
//! Two ways, and they meet at one surface. A deployment may register a published pack directly
//! through [`DaemonState::put_context_pack`](super::state::DaemonState::put_context_pack) — the
//! out-of-band administration route that predates the compiler — or a caller may compile one,
//! and [`compile`] then **writes through that same surface**, which is what this module
//! predicted it would: "when the compiler lands it will write through the same surface, and
//! nothing in this file changes." Nothing in [`expand`] did.
//!
//! A compiled pack is registered before it is answered with, so the `ctx_*` on the wire is
//! immediately navigable and the manifest's promise — "`recoverable_by` carrying the exact
//! `ctx_*` that record's query resolves to" — is true of this daemon at the moment it is made,
//! not at some later administrative step.
//!
//! # The typed outcome table, and where each row is decided
//!
//! RFC 0028 gives `context.expand` a normative outcome per condition. Every row is here,
//! and none is decided by a code this operation's `errors` clause does not admit:
//!
//! | Condition | Outcome | Where |
//! |---|---|---|
//! | the named pack is not one this daemon holds | `CapabilityDenied` | [`expand`], RFC 0027 X2 — never a distinguishable not-found |
//! | the envelope names a snapshot other than the one the pack is stated against | `StaleSnapshot` | [`stale_snapshot_check`], and see it for the two `task.resume` tests an expansion deliberately does not run |
//! | `anchor` does not resolve in the parent | `MalformedRequest` | [`expand`] |
//! | `relation` is not a member of `ExpansionRelation` | `MalformedRequest` | the codec, before this family runs — a closed enum's decode |
//! | the relation is undefined for the anchor, or the engine cannot compute it | `UnsupportedSemanticFeature` | [`expand`] |
//! | the target content is redacted, purged, summarized, or lost | **success**, with a `redaction` omission and the parent's own `redactions[]` stub | [`expand`] |
//! | the relation is defined and yields no items | **success**, empty selection and empty manifest | [`expand`] |
//! | the budget is exhausted before the expansion completes | **success** with a smaller child whose manifest records the shortfall, or — below the smallest conforming child — `BudgetExhausted` with nothing published | [`continuum_context::budget`], called from [`expand`] |
//!
//! The last row has both of RFC 0028's branches since PR-11/IMPL-06 (bn-38p2) — "either
//! nothing is published, or a child pack is published whose manifest records the shortfall
//! with reason `budget`". Which one a call gets is decided by the packer and not by this
//! module: it packs a smaller child while one exists, and the branch flips where even the
//! complete manifest beside an empty selection is larger than the ceiling, because the
//! manifest is "never the thing a budget squeezes out (INV-007)". That refusal carries a
//! typed `non_resumable_reason` (SD-13) rather than a bare code; the packed answer carries
//! the shortfall as an ordinary `budget` omission on the pack *and* on the envelope, which
//! is how a caller tells the two apart without reading a status code twice.
//!
//! `depth` of zero is `MalformedRequest`: the IDL types it `U32 optional`, absence means 1,
//! and neither RFC gives zero a meaning — a value inside the type and outside the
//! vocabulary, which is the same reading that makes an unknown enum member malformed.
//!
//! # What an expansion answers with, and why the manifest is not optional
//!
//! The response carries the child pack, and the *envelope* carries the child's manifest as
//! `omissions[]`:
//!
//! > The result envelope's `omissions` list (`Omission { reason, subject, recoverable_by }`)
//! > is the wire projection of the same facts and MUST agree with the pack's manifest
//! > record for record.
//! >
//! > — RFC 0028, "Omission manifest"
//!
//! So the two travel together on every admitted answer, and the projection is mechanical:
//! one envelope omission per manifest record, the same reason token, and `recoverable_by`
//! carrying **the exact `ctx_*` that record's query resolves to** — derived by the same
//! function that derived this child's own identity, so what the manifest promises and what
//! a following `context.expand` returns are one value rather than two that agree today.
//!
//! # The compile's typed outcome table
//!
//! | Condition | Outcome | Where |
//! |---|---|---|
//! | no compile source is registered for the named `evidence_root` | `UnsupportedSemanticFeature` | [`compile`] — uniform, so it is not an existence oracle |
//! | the envelope names a snapshot other than the source's | `StaleSnapshot` | [`stale_snapshot_check`] |
//! | `question` is empty, untrimmed, or not printable ASCII | `MalformedRequest` | [`compile`], via `Question::compiled` |
//! | a requested guarantee is not a member of the closed thirteen | `MalformedRequest` | [`compile`] — "fail closed on unrecognized tokens" (RFC 0028) |
//! | the registered source names no root, or the compile has nothing to slice | `InsufficientEvidence` | [`compile`] |
//! | the pipeline refuses the registered input (a redacted root, an unanchored residual, …) | `UnsupportedSemanticFeature` | [`compile_fault`] |
//! | the assembled root would not reconcile, or violates its profile | `UnsupportedSemanticFeature` | [`pack_fault`] — nothing is published |
//! | the root selects more items than `output_policy.max_nodes` | `BudgetExhausted` | [`pack_fault`], from `PackError::OverNodeCeiling` — nothing is published |
//! | the root document is larger than `budget.bytes` | `BudgetExhausted` | [`compile`] — nothing is published |
//! | the result payload, in the negotiated encoding, is larger than `output_policy.max_bytes` | `BudgetExhausted` | [`compile`] — nothing is published |
//! | otherwise | **success**, with the pack, the verdict, and the manifest as envelope omissions | [`compile`] |
//!
//! # Over a ceiling: a typed refusal, and the rule that picks it
//!
//! Until bn-2ga1c this handler read no ceiling at all: a compile under `budget.bytes = 512`
//! answered `Ok` with a 23 KiB pack, while the IDL offered `BudgetExhausted` in the operation's
//! `errors` clause and no code took it. The ceilings are now read with the same helpers the
//! other two readers use — [`output::Ceiling::of_budget`] (`context.expand`'s `budget.bytes`),
//! [`output::Ceiling::of`] and [`output::measure`] (`task.status`'s `output_policy.max_bytes`),
//! [`output::max_nodes`] — and a ceiling admits an answer of exactly its own size (`<=`).
//!
//! RFC 0028 gives an over-budget pack two admissible answers and forbids a third:
//!
//! > **A guaranteed core is never truncated.** If the items required by a claimed guarantee
//! > alone exceed the budget, the compiler MUST either drop the guarantee — recording the drop
//! > as an omission with reason `budget` — or fail with `BudgetExhausted` carrying a
//! > continuation. It MUST NOT publish a pack that claims a guarantee whose supporting items
//! > were trimmed.
//! >
//! > — RFC 0028, "Budgets and packing"
//!
//! > The answer is a smaller pack with a larger manifest, or `BudgetExhausted` with a
//! > continuation.
//! >
//! > — RFC 0028, "Rejected alternatives"
//!
//! A root's selection here is stage 2's backward slice with stage 9 disabled, and stage 2 is
//! what licenses `CausallyClosed`: every selected item is core. So the budget rule is the one
//! that governs, and this handler takes its `BudgetExhausted` branch — the IDL's declared code,
//! carried with the typed `non_resumable_reason` (SD-13) exactly as `context.expand`'s
//! below-the-floor refusal is. It never takes the forbidden third branch: nothing is truncated,
//! nothing is dumped, and a refusal registers no pack (the ceilings are decided before
//! [`DaemonState::put_context_pack`](super::state::DaemonState::put_context_pack) runs), so this
//! `@mutation` commits nothing it then reports as failed.
//!
//! The other admissible branch — drop the guarantee and publish a smaller root with a larger
//! manifest — needs a root packer. [`continuum_context::budget::BudgetPacker`] packs expansion
//! children only, which claim no guarantee; a root packer must also decide which guarantee to
//! drop and record it. That is a packer bone, not a wire bone. When it lands it narrows the
//! refusal to "below the smallest conforming root", as `context.expand`'s already is, and no
//! wire shape changes.
//!
//! `content_budget.nodes` records the stated `max_nodes` (the schema: "Graph-node ceiling of
//! the IDL's OutputPolicy.max_nodes"), and a returned graph node is a `selected[]` item.
//!
//! # What is declined here, and why
//!
//! **No pack is published into the reference store — and the identity question that decision
//! left open is now settled** (bn-1y4qc). This module recorded the open question at IMPL-04:
//! "a store handle is the content identity of the bytes (`ContentIdentifier::identify`), and a
//! pack's `context_id` is the identity of its *question* — two spellings for one artifact
//! unless something reconciles them, and reconciling them is a decision about pack assembly."
//! [`continuum_context::pack`]'s "Root assembly, and the identity question it settles" answers
//! it: they are two identities of two different things, and the pack carries **both** —
//! `context_id` for the question, `content_hash` for the bytes — so a store handle and a `ctx_*`
//! are one field lookup apart in either direction and no second identity is minted anywhere.
//! Publishing would therefore add a *record*, never an identity, and the record is derivable
//! from the pack; the handler holds `&ReferenceStore` and mutates nothing, which is the shape
//! that reading is. Both operations return the pack on the wire and name no `ArtifactRef`.

use continuum_context::accounting::Omitted;
use continuum_context::assurance::Assurance;
use continuum_context::budget::{BudgetError, BudgetPacker};
use continuum_context::compile::{CausalCompile, Compilation, CompileError};
use continuum_context::expansion::{
    Depth, ExpansionHandle, ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::guarantee::{Guarantee, RequestedGuarantees};
use continuum_context::omission::{
    IrretrievableReason, Manifest, OmissionReason as PackReason, OmissionRecord, Retrievability,
};
use continuum_context::pack::{
    self, ChildPack, PackError, PackProfile, RedactionStub, RootIdentity, RootPack,
};
use continuum_context::scope::ScopeVerdict;
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::target::{Question, Target};
use continuum_context::verdict::Verdict as PackVerdict;
use continuum_intent::canonical_json::Json;
use continuum_value::assurance::{
    AssuranceDimension, AssuranceLevel, DimensionEvidence,
    InconclusiveReason as ValueInconclusiveReason, UnsupportedReason,
};
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle as StoreHandle};
use continuum_workspace::publication::ReferenceStore;

use std::collections::{BTreeMap, BTreeSet};

use super::Services;
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::output;
use super::state::DaemonState;
use crate::protocol::envelope::{EvaluationVerdictValue, Omission, RequestEnvelope, Verdict};
use crate::protocol::operations::context::{
    ContextCompileRequest, ContextCompileResponse, ContextExpandRequest, ContextExpandResponse,
};
use crate::protocol::scalar::{ArtifactHandle, ContextHandle, Opaque, WorkspaceHandle};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{
    AssuranceClass, Encoding, ErrorCode, EvaluationVerdict, ExpansionRelation, InconclusiveReason,
    OmissionReason,
};

/// The `context` namespace's two operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// Held to `rule errors.common` ∪ each operation's `errors` clause by
/// `tests/daemon_context_operations.rs`, the way `observe::FAULTS` is by
/// `tests/daemon_evidence.rs`.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("context.compile", ErrorCode::InsufficientEvidence),
    ("context.compile", ErrorCode::MalformedRequest),
    ("context.compile", ErrorCode::StaleSnapshot),
    ("context.compile", ErrorCode::UnsupportedSemanticFeature),
    ("context.compile", ErrorCode::BudgetExhausted),
    ("context.expand", ErrorCode::CapabilityDenied),
    ("context.expand", ErrorCode::MalformedRequest),
    ("context.expand", ErrorCode::StaleSnapshot),
    ("context.expand", ErrorCode::UnsupportedSemanticFeature),
    ("context.expand", ErrorCode::BudgetExhausted),
];

/// The refusal detail for a root pack larger than the envelope's `budget.bytes`.
///
/// Also the typed `non_resumable_reason` (SD-13), as on `context.expand`'s refusal.
pub const COMPILE_OVER_BUDGET_BYTES: &str = "the compiled root pack is larger than the byte \
     budget this call ran under; a root's selection is its guaranteed core, which is never \
     truncated, and this deployment packs no smaller root";

/// The refusal detail for a compile answer larger than `output_policy.max_bytes`.
pub const COMPILE_OVER_MAX_BYTES: &str = "the compile's result payload, in the negotiated \
     encoding, is larger than output_policy.max_bytes; a root's selection is its guaranteed \
     core, which is never truncated, and this deployment packs no smaller root";

/// The refusal detail for a root that selects more items than `output_policy.max_nodes`.
pub const COMPILE_OVER_MAX_NODES: &str = "the compiled root pack selects more items than \
     output_policy.max_nodes; a root's selection is its guaranteed core, which is never \
     truncated, and this deployment packs no smaller root";

/// A published Context Pack this daemon can navigate, and the groups its manifest names.
///
/// The document is held *parsed*: a pack whose JSON this daemon cannot read is refused when
/// it is registered rather than when a caller asks, which is the same direction
/// [`DaemonState::stage`](super::state::DaemonState::stage) takes with content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPackRecord {
    document: Json,
    snapshot: WorkspaceHandle,
    anchors: BTreeSet<Name>,
    retrieved: BTreeSet<Name>,
    payloads: BTreeMap<ExpansionQuery, ExpansionPayload>,
}

impl ContextPackRecord {
    /// Register a pack and the expansion payloads its manifest accounts for.
    ///
    /// The constructor is where the expansion graph is made well-formed, because the
    /// conservation property expansion rests on — no item selected twice, no item reachable
    /// from nowhere — is a property of *this* structure and not of the walk over it:
    ///
    /// - the document carries the schema's required keys and a `ctx_*` `context_id`;
    /// - every payload's anchor resolves, either in the parent's own anchors or as an item
    ///   another payload retrieves (which is how depth > 1 reaches anything at all);
    /// - no item identity is held by two payloads, and none collides with a parent
    ///   `selected[].id` — so an expansion cannot return one item twice, and the parent's
    ///   selection and its manifest stay disjoint, which is what makes the counting
    ///   equation meaningful across a pack family rather than only inside one pack.
    ///
    /// # Errors
    ///
    /// [`ContextPackError`], naming which of those it is.
    pub fn new(
        document: Json,
        snapshot: WorkspaceHandle,
        payloads: impl IntoIterator<Item = ExpansionPayload>,
    ) -> Result<Self, ContextPackError> {
        pack::required_keys_present(&document).map_err(ContextPackError::Malformed)?;
        pack::identity_of(&document).map_err(ContextPackError::Malformed)?;
        pack::budget_bytes_of(&document).map_err(ContextPackError::Malformed)?;
        let anchors = pack::anchors(&document).map_err(ContextPackError::Malformed)?;

        let mut held: BTreeSet<Name> = BTreeSet::new();
        let mut by_query: BTreeMap<ExpansionQuery, ExpansionPayload> = BTreeMap::new();
        for payload in payloads {
            for item in payload.items() {
                if anchors.contains(item.id()) || !held.insert(item.id().clone()) {
                    return Err(ContextPackError::RepeatedItem {
                        id: item.id().clone(),
                    });
                }
            }
            let Some(query) = payload.query().cloned() else {
                return Err(ContextPackError::UnreachableGroup);
            };
            if by_query.insert(query.clone(), payload).is_some() {
                return Err(ContextPackError::RepeatedQuery { query });
            }
        }
        for query in by_query.keys() {
            if !anchors.contains(query.anchor()) && !held.contains(query.anchor()) {
                return Err(ContextPackError::DanglingAnchor {
                    anchor: query.anchor().clone(),
                });
            }
        }
        Ok(Self {
            document,
            snapshot,
            anchors,
            retrieved: held,
            payloads: by_query,
        })
    }

    /// Register a pack together with the irretrievable groups its manifest names.
    ///
    /// Split from [`ContextPackRecord::new`] because an irretrievable group has no query to
    /// key it by and therefore cannot travel in the same map: the pack that dropped it can
    /// still be expanded *at* the anchor, and what comes back is the redaction row of the
    /// typed-outcome table.
    ///
    /// # Errors
    ///
    /// As [`ContextPackRecord::new`].
    pub fn with_irretrievable(
        mut self,
        anchor: Name,
        relation: PackRelation,
        record: OmissionRecord,
    ) -> Result<Self, ContextPackError> {
        let Retrievability::Irretrievable(_) = record.retrievability() else {
            return Err(ContextPackError::RetrievableGroup);
        };
        let query = ExpansionQuery::new(relation, anchor);
        if !self.resolves(query.anchor()) {
            return Err(ContextPackError::DanglingAnchor {
                anchor: query.anchor().clone(),
            });
        }
        let payload = ExpansionPayload::irretrievable(record)
            .map_err(|_| ContextPackError::RetrievableGroup)?;
        if self.payloads.insert(query.clone(), payload).is_some() {
            return Err(ContextPackError::RepeatedQuery { query });
        }
        Ok(self)
    }

    /// Whether an anchor resolves here: a `selected[].id` or an advertised query's anchor
    /// in the pack itself, or an item some registered group retrieves — which is how an
    /// expansion deeper than one hop reaches anything.
    #[must_use]
    pub fn resolves(&self, anchor: &Name) -> bool {
        self.anchors.contains(anchor) || self.retrieved.contains(anchor)
    }

    /// The pack document, as published.
    #[must_use]
    pub const fn document(&self) -> &Json {
        &self.document
    }

    /// The snapshot this pack is stated against.
    #[must_use]
    pub const fn snapshot(&self) -> &WorkspaceHandle {
        &self.snapshot
    }

    /// The manifest this pack published: one record per registered group.
    ///
    /// Derived from the payloads rather than stored beside them, so a manifest that
    /// disagreed with what the daemon can actually return has no spelling here.
    ///
    /// # Errors
    ///
    /// [`ContextPackError::Partition`] if two groups cannot be merged into one partition.
    pub fn manifest(&self) -> Result<Manifest, ContextPackError> {
        Manifest::merged(
            self.payloads
                .values()
                .map(|payload| payload.record().clone()),
        )
        .map_err(|_| ContextPackError::Partition)
    }
}

/// Everything a deployment must register before this daemon can compile a pack from an
/// evidence root, and nothing this daemon could have derived itself.
///
/// The split is deliberate and is the whole honesty of the wiring. What is registered is the
/// **projection**: the candidate order stage 2 slices (whose producer, `continuum-cir`, is a
/// PR-17 scaffold), a body for every candidate (whose typed constructors are PR-11/IMPL-02 and
/// IMPL-03, but whose *contents* come from the evidence a deployment holds), and the answer
/// header a pack states about the evaluation it explains (`intent`, `snapshot`,
/// `semantic_epoch`, `evidence`, `replay`, `verdict`, `assurance`). What is **not** registered
/// is anything a checker decides: the selection, the guarantees, the manifest, the counting
/// equation and the document are the pipeline's, and a deployment cannot pre-state any of them.
///
/// A body for *every* node of the order is required rather than only for the ones a compile
/// happens to select, because which those are is the compiler's answer and not the
/// registrant's: a source that supplied bodies for a guessed selection would decide the
/// selection. It is also what makes INV-007 answerable — an omitted candidate must be
/// *expandable*, and an expansion returns items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCompileSource {
    compile: CausalCompile,
    roots: BTreeSet<Name>,
    residual: ExpansionQuery,
    items: BTreeMap<Name, SelectedItem>,
    snapshot: WorkspaceHandle,
    semantic_epoch: String,
    intent: StoreHandle,
    evidence: Vec<StoreHandle>,
    replay: Option<continuum_context::replay::ReplayRef>,
    verdict: Option<PackVerdict>,
    assurance: Assurance,
    profile: PackProfile,
    redactions: Vec<RedactionStub>,
}

impl ContextCompileSource {
    /// Register the projection one evidence root compiles from.
    ///
    /// # Errors
    ///
    /// [`ContextPackError::UnbodiedCandidate`] when a node of the order has no item body, and
    /// [`ContextPackError::BodyFromNowhere`] when a body names something the order does not
    /// carry or disagrees with that node's declared kind — the two directions of "the
    /// registered bodies are exactly the candidate universe", checked at registration for the
    /// reason [`ContextPackRecord::new`] checks its own: a source this daemon could not compile
    /// from is refused when it is registered, never when a caller asks.
    pub fn new(
        compile: CausalCompile,
        roots: impl IntoIterator<Item = Name>,
        residual: ExpansionQuery,
        items: impl IntoIterator<Item = SelectedItem>,
        header: CompileHeader,
    ) -> Result<Self, ContextPackError> {
        let mut bodies: BTreeMap<Name, SelectedItem> = BTreeMap::new();
        for item in items {
            let Some(kind) = compile.order().kind(item.id()) else {
                return Err(ContextPackError::BodyFromNowhere {
                    id: item.id().clone(),
                });
            };
            if kind != item.kind() {
                return Err(ContextPackError::BodyFromNowhere {
                    id: item.id().clone(),
                });
            }
            bodies.insert(item.id().clone(), item);
        }
        for (id, _) in compile.order().nodes() {
            if !bodies.contains_key(id) {
                return Err(ContextPackError::UnbodiedCandidate { id: id.clone() });
            }
        }
        Ok(Self {
            compile,
            roots: roots.into_iter().collect(),
            residual,
            items: bodies,
            snapshot: header.snapshot,
            semantic_epoch: header.semantic_epoch,
            intent: header.intent,
            evidence: header.evidence,
            replay: header.replay,
            verdict: header.verdict,
            assurance: header.assurance,
            profile: header.profile,
            redactions: header.redactions,
        })
    }

    /// The snapshot every pack compiled from this source is stated against.
    #[must_use]
    pub const fn snapshot(&self) -> &WorkspaceHandle {
        &self.snapshot
    }

    /// The pipeline this source configures.
    #[must_use]
    pub const fn compile(&self) -> &CausalCompile {
        &self.compile
    }

    /// Every registered item body, in canonical identity order.
    pub fn items(&self) -> impl Iterator<Item = &SelectedItem> {
        self.items.values()
    }

    /// The registered body of one candidate.
    #[must_use]
    pub fn item(&self, id: &Name) -> Option<&SelectedItem> {
        self.items.get(id)
    }
}

/// The answer header a compiled pack states, registered beside the projection.
///
/// A struct rather than nine parameters because every field is a *fact about the evaluation the
/// pack explains* rather than about the compile, and grouping them keeps that boundary visible
/// at the call site: nothing here is derived, and nothing here decides a guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileHeader {
    /// The sealed snapshot the pack is stated against.
    pub snapshot: WorkspaceHandle,
    /// The pinned semantic epoch.
    pub semantic_epoch: String,
    /// The `in_*` contract the question is about.
    pub intent: StoreHandle,
    /// The `ev_*` roots.
    pub evidence: Vec<StoreHandle>,
    /// The `crash_*` replay handle, where the deployment holds one.
    pub replay: Option<continuum_context::replay::ReplayRef>,
    /// The verdict of the evaluation, when the pack explains one.
    ///
    /// `None` is a pack whose question is not about an evaluation outcome, and RFC 0028 is
    /// explicit about what that pack carries: "MUST carry `inconclusive` with the reason naming
    /// why no verdict is available […] It MUST NOT default to `satisfied`." [`compile`] pairs
    /// `None` with [`Compilation::inconclusive_reason`] and refuses `InsufficientEvidence` when
    /// there is no reason either, because a verdict this daemon cannot name is not one it may
    /// invent.
    pub verdict: Option<PackVerdict>,
    /// The `{class, envelope}` block the result reports. Its `observer` dimension is **not**
    /// read from here — see [`observer_dimension`].
    pub assurance: Assurance,
    /// Which of RFC 0028's four profiles the compile is held to.
    pub profile: PackProfile,
    /// The withheld-content stubs the deployment's field policy earned.
    pub redactions: Vec<RedactionStub>,
}

/// A pack, or a group, this daemon will not register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextPackError {
    /// A node of the causal order has no registered item body.
    UnbodiedCandidate {
        /// The candidate.
        id: Name,
    },
    /// A registered body names a candidate the order does not carry, or a different kind.
    BodyFromNowhere {
        /// The body's identity.
        id: Name,
    },
    /// The document is not a pack this daemon can navigate.
    Malformed(PackError),
    /// A group's anchor resolves neither in the pack nor in another group's items.
    DanglingAnchor {
        /// The anchor.
        anchor: Name,
    },
    /// Two groups claim one item, or a group claims an item the pack already selected.
    RepeatedItem {
        /// The item.
        id: Name,
    },
    /// Two groups answer one query.
    RepeatedQuery {
        /// The query.
        query: ExpansionQuery,
    },
    /// An expandable group was registered without the query that reaches it.
    UnreachableGroup,
    /// An irretrievable group was registered as a retrievable one.
    RetrievableGroup,
    /// The registered groups do not form one partition.
    Partition,
}

impl core::fmt::Display for ContextPackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnbodiedCandidate { id } => write!(
                f,
                "the candidate `{id}` has no registered item body, so a compile that selected \
                 or omitted it could publish neither the item nor its expansion"
            ),
            Self::BodyFromNowhere { id } => write!(
                f,
                "the registered body `{id}` names no candidate of this order, or a candidate of \
                 another kind"
            ),
            Self::Malformed(error) => write!(f, "{error}"),
            Self::DanglingAnchor { anchor } => write!(
                f,
                "no `{anchor}` resolves in this pack or in anything expanding it reaches"
            ),
            Self::RepeatedItem { id } => {
                write!(
                    f,
                    "`{id}` is accounted for twice; an expansion would return it twice"
                )
            }
            Self::RepeatedQuery { query } => {
                write!(f, "two groups answer the expansion `{query}`")
            }
            Self::UnreachableGroup => {
                f.write_str("an expandable group carries no query, so nothing reaches it")
            }
            Self::RetrievableGroup => {
                f.write_str("an irretrievable group is registered with `with_irretrievable`")
            }
            Self::Partition => f.write_str("the registered groups do not partition"),
        }
    }
}

impl core::error::Error for ContextPackError {}

impl OperationFamily for ContextFamily {
    fn namespace(&self) -> &'static str {
        "context"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        // Pure in the arguments, and it must be: `scope` runs before admission, so it
        // cannot resolve the named pack to the snapshot that pack is stated against. What a
        // `context` request *names* is a Context Pack, and for `compile` the evidence root
        // it compiles from; the snapshot behind a pack is checked after admission, in
        // `expand`, where reading state is allowed.
        match arguments {
            Arguments::ContextExpand(_) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![ArtifactClass::ContextPack.token()],
            },
            Arguments::ContextCompile(_) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![
                    ArtifactClass::ContextPack.token(),
                    ArtifactClass::Evidence.token(),
                ],
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        _store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::ContextExpand(request) => expand(call.envelope, request, state),
            Arguments::ContextCompile(request) => compile(
                call.envelope,
                request,
                state,
                services.negotiated().encoding(),
            ),
            // Unreachable: the dispatcher checked shape agreement before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

/// `context.compile` — run the landed pipeline over a registered projection, assemble the root
/// pack, register it for expansion, and answer with it.
///
/// The order of the steps is the order of the refusals: everything that is about the *request*
/// is decided before any stage runs, so a malformed question or an unknown guarantee token
/// never costs a compile, and everything that is about the *answer* is decided before anything
/// is registered, so a pack that would not reconcile is never navigable.
fn compile(
    envelope: &RequestEnvelope,
    request: &ContextCompileRequest,
    state: &mut DaemonState,
    encoding: Encoding,
) -> Result<Effect, Fault> {
    // Uniform in the root, so it answers the same whether or not this daemon holds one: the
    // absent producer is `continuum-cir`, which is a property of the deployment and not of the
    // caller's scope, and a distinguishable answer here would be the existence oracle RFC 0027
    // X2 closes elsewhere in this file.
    let source = state
        .compile_source(&request.evidence_root)
        .ok_or_else(|| {
            Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "this daemon holds no candidate order for that evidence root; stage 2's input \
                 is a CIR causal order and no subsystem here derives one from an evidence graph",
            )
        })?
        .clone();

    stale_snapshot_check(envelope, source.snapshot())?;

    let question = Question::compiled(&request.question).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "`question` is empty, untrimmed, or carries a character outside printable ASCII; a \
             question is part of the pack's identity and has one spelling",
        )
    })?;
    // "Fail closed on unrecognized tokens" (RFC 0028, "Versioning and revision"): the wire type
    // is `list<String>` (F6) over a closed thirteen-member vocabulary, so an unknown member is
    // `MalformedRequest` and is never read as absent.
    let requested = requested_guarantees(request)?;

    if source.roots.is_empty() {
        return Err(Fault::new(
            ErrorCode::InsufficientEvidence,
            "the registered projection for that evidence root names no root to slice from",
        ));
    }

    let compilation = source
        .compile
        .run(&source.roots, &source.residual)
        .map_err(compile_fault)?;

    // --- the answer header ---------------------------------------------------------------
    let target = Target::new(source.intent.clone(), question).map_err(|_| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the registered projection names an intent that is not an `in_*` contract",
        )
    })?;
    let verdict = match source.verdict {
        Some(verdict) => verdict,
        None => PackVerdict::Inconclusive(compilation.inconclusive_reason().ok_or_else(|| {
            Fault::new(
                ErrorCode::InsufficientEvidence,
                "the registered projection states no evaluation verdict and no stage refused, \
                 so there is no typed reason to carry and a verdict may not be defaulted",
            )
        })?),
    };
    let assurance = Assurance::new(
        source.assurance.class(),
        source.assurance.envelope().clone().with(
            AssuranceDimension::Observer,
            observer_dimension(&compilation),
        ),
    );

    let selected: Vec<SelectedItem> = compilation
        .selected()
        .iter()
        .map(|id| {
            source.items.get(id).cloned().ok_or_else(|| {
                // Unreachable through `ContextCompileSource::new`, which refuses an unbodied
                // candidate at registration; typed rather than unwrapped for the reason
                // `malformed_pack` gives.
                Fault::new(
                    ErrorCode::UnsupportedSemanticFeature,
                    "the compile selected a candidate this daemon holds no item body for",
                )
            })
        })
        .collect::<Result<_, _>>()?;
    let unachieved: Vec<Guarantee> = requested
        .members()
        .into_iter()
        .filter(|guarantee| !compilation.guarantees().members().contains(guarantee))
        .collect();

    let identity = RootIdentity::derive::<Blake3Hasher>(
        &target,
        source.snapshot.as_str(),
        &source.semantic_epoch,
        &source.evidence,
    );
    let assembly = RootPack {
        identity: &identity,
        snapshot: source.snapshot.as_str(),
        semantic_epoch: &source.semantic_epoch,
        target: &target,
        verdict,
        assurance: &assurance,
        selected: &selected,
        manifest: compilation.accounting().manifest(),
        candidates: compilation.accounting().candidate_count(),
        unachieved: &unachieved,
        evidence: &source.evidence,
        replay: source.replay.as_ref(),
        guarantees: compilation.guarantees(),
        redactions: &source.redactions,
        profile: source.profile,
        nodes: output::max_nodes(envelope),
    };
    let document = assembly.to_json::<Blake3Hasher>().map_err(pack_fault)?;
    let published = assembly.published_manifest().map_err(pack_fault)?;

    // --- the ceilings: decided on the finished answer, before anything is registered ------
    //
    // The byte ceiling of `budget.bytes` is read against the pack document — the quantity
    // `content_budget.bytes` measures and `context.expand` admits against — and
    // `output_policy.max_bytes` against the result payload in the negotiated encoding, as
    // `daemon::output` does for `task.status`. `max_nodes` was refused by the writer above
    // (`PackError::OverNodeCeiling`, mapped in `pack_fault`). Every over-ceiling outcome is
    // `BudgetExhausted` with nothing published — see "Over a ceiling" in this module's
    // documentation for the RFC 0028 rule that picks this branch.
    if !output::Ceiling::of_budget(envelope).admits(document.to_canonical_bytes().len() as u64) {
        return Err(Fault::exhausted(COMPILE_OVER_BUDGET_BYTES));
    }
    let context = ContextHandle::new(&identity.to_string()).map_err(|_| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the derived pack identity is not a well-formed context handle",
        )
    })?;
    let response = ContextCompileResponse {
        context: context.clone(),
        pack: Opaque::from_bytes(document.to_canonical_bytes()),
    };
    let ceiling = output::Ceiling::of(envelope);
    if ceiling.stated().is_some() && !ceiling.admits(output::measure(&response, encoding)?) {
        return Err(Fault::exhausted(COMPILE_OVER_MAX_BYTES));
    }

    // --- registration: the compiled pack is navigable the moment it is answered with -------
    let record = ContextPackRecord::new(
        document.clone(),
        source.snapshot.clone(),
        payloads(&source, &compilation)?,
    )
    .map_err(|_| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the assembled pack is not one this daemon could then navigate, so it is not \
             published at all",
        )
    })?;
    state.put_context_pack(context.clone(), record);

    Ok(Effect::new(
        Payload::ContextCompile(response),
        // > `context.compile` is `@mutation @task_starting` with `authority read` and verdict
        // > `EvaluationVerdictValue`; that verdict MUST equal the pack's `verdict`, and its
        // > `assurance_class` MUST equal the pack's `assurance.class`.
        // >
        // > — RFC 0028, "Wire surface"
        //
        // Both are read off the values the document was written from, so the two cannot drift:
        // there is one `verdict` and one `class` in this function, spelled twice.
        Nullable::Value(Verdict::Evaluation(EvaluationVerdictValue {
            verdict: wire_verdict(verdict),
            inconclusive_reason: match verdict.inconclusive_reason() {
                Some(reason) => Optional::Present(wire_reason_token(reason)),
                None => Optional::Absent,
            },
            assurance_class: wire_class(assurance.class()),
        })),
    )
    .with_omissions(project(&published, identity.handle())))
}

/// The `guarantees` request list, parsed against the closed vocabulary.
fn requested_guarantees(request: &ContextCompileRequest) -> Result<RequestedGuarantees, Fault> {
    let Optional::Present(tokens) = &request.guarantees else {
        return Ok(RequestedGuarantees::none());
    };
    let mut members = Vec::with_capacity(tokens.len());
    for token in tokens {
        members.push(Guarantee::from_wire_str(token).ok_or_else(|| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "`guarantees` names a token outside the closed thirteen-member vocabulary; \
                 forward compatibility is achieved by rejecting, never by ignoring",
            )
        })?);
    }
    Ok(RequestedGuarantees::of(members))
}

/// The expansion payloads a compiled pack is registered with: one per expandable manifest
/// record, carrying exactly the items that record counts.
///
/// The grouping is the compile's own — [`Compilation::omissions`] is the ledger the manifest
/// was read off — so a record and the items an expansion returns for it cannot be two answers.
/// Irretrievable groups get no payload and need none: they advertise no query, so no anchor in
/// the published document reaches them.
fn payloads(
    source: &ContextCompileSource,
    compilation: &Compilation,
) -> Result<Vec<ExpansionPayload>, Fault> {
    let mut groups: BTreeMap<(SelectionKind, Omitted), Vec<SelectedItem>> = BTreeMap::new();
    for (id, omitted) in compilation.omissions() {
        let Omitted::Expandable { .. } = omitted else {
            continue;
        };
        let (Some(kind), Some(item)) = (source.compile.order().kind(id), source.items.get(id))
        else {
            return Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the compile omitted a candidate this daemon holds no item body for",
            ));
        };
        groups
            .entry((kind, omitted.clone()))
            .or_default()
            .push(item.clone());
    }
    let mut queries: BTreeSet<ExpansionQuery> = BTreeSet::new();
    let mut out = Vec::with_capacity(groups.len());
    for ((kind, omitted), mut items) in groups {
        let Omitted::Expandable { reason, query } = omitted else {
            continue;
        };
        // One expansion query answers one group, because `context.expand` resolves an answer by
        // its `(relation, anchor)` and two groups behind one query would make that resolution
        // ambiguous. `CausalCompile::run` takes a single residual query, so a compile whose
        // omissions span two kinds is refused here rather than registered as a pack advertising
        // a query this daemon could only half answer. A per-kind residual plan is the expansion
        // protocol's own bone, not this one's; what must not happen is a silent half-answer.
        if !queries.insert(query.clone()) {
            return Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "this compile's omissions span two selection kinds behind one residual \
                 expansion query, and one query answers one group",
            ));
        }
        items.sort();
        let count = u32::try_from(items.len()).map_err(|_| {
            Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "an omitted group is larger than an exact count can carry",
            )
        })?;
        out.push(
            ExpansionPayload::new(
                OmissionRecord::expandable(kind, count, reason, query),
                items,
            )
            .map_err(|_| {
                Fault::new(
                    ErrorCode::UnsupportedSemanticFeature,
                    "an omitted group's count and its items disagree",
                )
            })?,
        );
    }
    Ok(out)
}

/// What the pack's `assurance.envelope.observer` dimension says, decided by whether stage 6
/// ran and what the independent audit concluded.
///
/// This is the honest reconciliation bn-imhw2 flagged and left to this bone. `daemon::evidence`
/// writes `Unsupported("no-observer-projection")` on every `evidence.verify` answer, and that
/// was read as a statement about the *daemon*. It is not, and now cannot be mistaken for one:
/// the dimension is a statement about the answer that carries it. An `evidence.verify`
/// re-derivation projects no observer and says so; a compile that ran stage 6 over a registered
/// projection *does* project one, and names [`continuum_context::observer`] as the engine that
/// did — with the same token, `no-observer-projection`, on a compile that did not configure the
/// stage. One reason token, two answers, neither of them a claim about the whole daemon.
///
/// An audit that found the reduction unjustified is **not** a producer: RFC 0028 makes stage 6
/// "never a guarantee by itself", and a dimension that named an engine for a reduction the
/// independent audit refused would be crediting the filter with the audit's verdict.
fn observer_dimension(compilation: &Compilation) -> DimensionEvidence {
    let unsupported = |token: &str| {
        DimensionEvidence::Unsupported(
            UnsupportedReason::new(token).expect("a plain lowercase token"),
        )
    };
    match compilation.scope_verdict() {
        None => unsupported("no-observer-projection"),
        Some(ScopeVerdict::Inapplicable(_)) => unsupported("observer-projection-inapplicable"),
        Some(ScopeVerdict::Unjustified(_)) => unsupported("observer-reduction-unjustified"),
        Some(ScopeVerdict::Justified { .. }) => DimensionEvidence::produced(
            "continuum-context::observer",
            "stage 6 scoped the selection to the intent's named observers, and the independent \
             scope audit found the reduction justified (INV-013)",
        )
        .expect("a non-empty engine and summary"),
    }
}

/// The wire spelling of a pack verdict — exhaustive, so a member added to either closed
/// four-token vocabulary is a compile error here rather than a mismatch on the wire.
const fn wire_verdict(verdict: PackVerdict) -> EvaluationVerdict {
    match verdict {
        PackVerdict::Satisfied => EvaluationVerdict::Satisfied,
        PackVerdict::Refuted => EvaluationVerdict::Refuted,
        PackVerdict::Deadlock => EvaluationVerdict::Deadlock,
        PackVerdict::Inconclusive(_) => EvaluationVerdict::Inconclusive,
    }
}

/// The wire spelling of an INV-008 reason — exhaustive, for [`wire_verdict`]'s reason.
const fn wire_reason_token(reason: ValueInconclusiveReason) -> InconclusiveReason {
    match reason {
        ValueInconclusiveReason::Unsupported => InconclusiveReason::Unsupported,
        ValueInconclusiveReason::ResourceExhausted => InconclusiveReason::ResourceExhausted,
        ValueInconclusiveReason::EngineError => InconclusiveReason::EngineError,
        ValueInconclusiveReason::InsufficientTelemetry => InconclusiveReason::InsufficientTelemetry,
        ValueInconclusiveReason::AbstractionAmbiguity => InconclusiveReason::AbstractionAmbiguity,
        ValueInconclusiveReason::IncompleteProofSearch => InconclusiveReason::IncompleteProofSearch,
    }
}

/// The wire spelling of an assurance class — exhaustive, for [`wire_verdict`]'s reason.
const fn wire_class(class: AssuranceLevel) -> AssuranceClass {
    match class {
        AssuranceLevel::Observed => AssuranceClass::Observed,
        AssuranceLevel::Sampled => AssuranceClass::Sampled,
        AssuranceLevel::Bounded => AssuranceClass::Bounded,
        AssuranceLevel::Validated => AssuranceClass::Validated,
        AssuranceLevel::Proved => AssuranceClass::Proved,
    }
}

/// The wire answer to a pipeline that refused its registered input.
///
/// Every arm is `UnsupportedSemanticFeature` and that is a decision rather than a default: each
/// names a projection this daemon cannot compile from, which is the deployment's fact, not the
/// caller's — the caller supplied a root, a question and a guarantee list, all of which were
/// admitted before the pipeline ran. A `MalformedRequest` here would blame the caller for a
/// registration they did not make.
fn compile_fault(error: CompileError) -> Fault {
    match error {
        CompileError::RedactedRoot { .. } => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "field policy withholds a root of the registered projection; a redacted root is \
             not a root, and slicing from one leaks the shape of what it dropped",
        ),
        CompileError::AnchorNotSelected { .. } => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the registered residual expansion query anchors at something this compile does \
             not publish, and expansion is navigation over a published pack",
        ),
        CompileError::Causal(_)
        | CompileError::NotACorrespondenceKind { .. }
        | CompileError::Accounting(_)
        | CompileError::Trail(_)
        | CompileError::Unreachable => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the registered projection is not one this daemon's pipeline can compile",
        ),
    }
}

/// The wire answer to a root the assembler refused to write.
///
/// Nothing is published on any arm, which is the point: INV-007's counting equation and the
/// profile's content constraints are checked *before* the document exists, so a pack that could
/// not name what it dropped is not a smaller answer but no answer.
fn pack_fault(error: PackError) -> Fault {
    match error {
        PackError::Unreconciled { .. } => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the assembled pack's selection and manifest do not account for its candidate set; \
             a pack that cannot name what it dropped is malformed, not compact (INV-007)",
        ),
        PackError::ProfileViolation { .. } => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the compile does not meet the content constraints of the pack profile it was \
             registered under (RFC 0028, \"Pack profiles\")",
        ),
        PackError::ReplayPreservingWithoutReplay => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "rule C2: a pack claiming `ReplayPreserving` MUST carry a non-null `replay`",
        ),
        PackError::OverNodeCeiling { .. } => Fault::exhausted(COMPILE_OVER_MAX_NODES),
        _ => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the registered projection does not assemble into a conforming root pack",
        ),
    }
}

/// `context.expand` — follow one expansion handle and answer with the child pack and the
/// manifest that is still outstanding.
fn expand(
    envelope: &RequestEnvelope,
    request: &ContextExpandRequest,
    state: &mut DaemonState,
) -> Result<Effect, Fault> {
    // Content this daemon does not hold is a denial, never a not-found (RFC 0027 X2): a
    // distinguishable "no such pack" would answer whether a `ctx_*` outside this caller's
    // scope exists, which is the existence oracle that rule closes.
    let record = state
        .context_pack(&request.context)
        .ok_or_else(Fault::denied)?
        .clone();

    // The one snapshot test an expansion runs — see [`stale_snapshot_check`] for the two
    // `task.resume` runs that this deliberately does not.
    stale_snapshot_check(envelope, record.snapshot())?;

    let anchor = Name::new(&request.anchor).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the anchor is not a canonical identifier, so nothing in the pack carries it",
        )
    })?;
    let depth = Depth::from_optional(request.depth.value().copied()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "an expansion depth of zero reaches nothing; omit `depth` for the default of one",
        )
    })?;
    let query = ExpansionQuery::new(crosswalk(request.relation), anchor);

    let Some(payload) = record.payloads.get(&query) else {
        // Two different refusals, and the difference is which half of the request is
        // wrong. RFC 0028: an anchor that does not resolve is `MalformedRequest`; a
        // relation that is undefined for the anchor, or that the engine cannot compute,
        // is `UnsupportedSemanticFeature`.
        if record.resolves(query.anchor()) {
            return Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "this relation is not one this daemon can follow from that anchor",
            ));
        }
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "the anchor does not resolve in the named pack",
        ));
    };

    // The redacted row: success, never an error. "An error carries no manifest and no
    // commitment, so the caller learns less than the redaction policy already permits them
    // to know" (RFC 0028's rejected alternatives). The child selects nothing, its manifest
    // carries the `redaction` record, and the parent's own `redactions[]` — where the typed
    // `Redacted(reason, commitment)` stub was recorded when the policy ran, before stage 1
    // — travels into the child with the other inherited keys.
    let (selected, residual) = match payload.record().retrievability() {
        Retrievability::Irretrievable(IrretrievableReason::Unsupported) => {
            return Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the content behind that anchor is not one this protocol version produces",
            ));
        }
        Retrievability::Irretrievable(IrretrievableReason::Redaction) => {
            (Vec::new(), vec![payload.record().clone()])
        }
        Retrievability::Expandable(_) => walk(&record, payload, depth),
    };

    let manifest = Manifest::merged(residual).map_err(|_| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the groups this expansion reaches do not form one manifest partition",
        )
    })?;

    let parent_id = pack::identity_of(record.document()).map_err(malformed_pack)?;
    let identity = ExpansionHandle::derive::<Blake3Hasher>(&parent_id, &query, depth)
        .map_err(|_| malformed_pack(PackError::NotAPackHandle))?;
    let question = continuum_context::expansion::ExpansionQuestion::of(&query, depth);
    let ceiling = ceiling(&record, envelope).map_err(malformed_pack)?;

    // RFC 0028's budget row, both branches, decided by the packer: a smaller child whose
    // manifest records the shortfall where one fits, and nothing published where none does.
    // The child this daemon hands to it is the *whole* answer — packing is the only thing
    // that may shrink it, and it may not shrink the manifest.
    let packed = BudgetPacker {
        full: ChildPack {
            parent: record.document(),
            identity: &identity,
            question: &question,
            selected: &selected,
            manifest: &manifest,
            ceiling,
        },
        shortfall: &query,
    }
    .pack::<Blake3Hasher>()
    .map_err(budget_fault)?;

    let context = ContextHandle::new(&identity.to_string()).map_err(|_| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the derived pack identity is not a well-formed context handle",
        )
    })?;
    // The manifest the *published* child carries, which is the pre-packing one plus whatever
    // the ceiling cost: "the wire projection of the same facts", record for record.
    let omissions = project(packed.manifest(), &parent_id);

    Ok(Effect::new(
        Payload::ContextExpand(ContextExpandResponse {
            context,
            parent: request.context.clone(),
            pack: Opaque::from_bytes(packed.document().to_canonical_bytes()),
        }),
        // `context.expand` declares no `verdict` clause — "consistent with returning a child
        // of an already-decided question" — and the dispatcher checks the agreement.
        Nullable::Null,
    )
    .with_omissions(omissions))
}

/// Follow the expansion graph from `payload`, up to `depth` hops.
///
/// Returns the items the child selects and the records that are still outstanding — the
/// groups reachable from what was selected but beyond the depth asked for. Those two are
/// the child's `selected[]` and its manifest, and together they account for exactly the
/// groups this walk touched: an item is selected or its group is named, never neither and
/// never both.
///
/// A group already visited is not visited twice. The registration guard makes a cycle
/// impossible to build in the first place — an item identity is held by one group — but a
/// walk that would return an item twice if one ever existed is a walk whose accounting
/// depends on a constructor's invariant holding, and this one does not.
fn walk<'a>(
    record: &'a ContextPackRecord,
    root: &'a ExpansionPayload,
    depth: Depth,
) -> (Vec<SelectedItem>, Vec<OmissionRecord>) {
    let mut selected: Vec<SelectedItem> = Vec::new();
    let mut residual: Vec<OmissionRecord> = Vec::new();
    let mut visited: BTreeSet<&ExpansionQuery> = BTreeSet::new();
    let mut current: Vec<&ExpansionPayload> = vec![root];

    for hop in 1..=depth.get() {
        let mut next: Vec<&ExpansionPayload> = Vec::new();
        for payload in current {
            let Some(query) = payload.query() else {
                // An irretrievable group reached transitively is named, never followed.
                residual.push(payload.record().clone());
                continue;
            };
            if !visited.insert(query) {
                continue;
            }
            selected.extend(payload.items().iter().cloned());
            for (onward_query, onward) in &record.payloads {
                if !payload
                    .items()
                    .iter()
                    .any(|item| item.id() == onward_query.anchor())
                {
                    continue;
                }
                if hop < depth.get() {
                    next.push(onward);
                } else {
                    residual.push(onward.record().clone());
                }
            }
        }
        if next.is_empty() {
            break;
        }
        current = next;
    }
    selected.sort();
    (selected, residual)
}

/// The wire projection of the child's manifest (RFC 0028, "Omission manifest").
///
/// One envelope omission per record, and `recoverable_by` carries the exact `ctx_*` the
/// record's query resolves to — derived here by the same function that derived the child's
/// own identity, at the default depth an omission's query implies.
fn project(
    manifest: &Manifest,
    parent: &continuum_workspace::artifact_path::ArtifactHandle,
) -> Vec<Omission> {
    manifest
        .records()
        .iter()
        .map(|record| Omission {
            reason: wire_reason(record.reason()),
            // Typed terms, never prose: the kind token the manifest partitions by, under
            // the `selected` field group it partitions (`rule envelope.no_prose`).
            subject: format!("selected.{}", record.kind()),
            recoverable_by: record
                .retrievability()
                .query()
                .map_or(Optional::Absent, |query| {
                    ExpansionHandle::derive::<Blake3Hasher>(parent, query, Depth::DEFAULT)
                        .ok()
                        .and_then(|handle| ArtifactHandle::new(&handle.to_string()).ok())
                        .map_or(Optional::Absent, Optional::Present)
                }),
        })
        .collect()
}

/// The byte ceiling this expansion runs under: the request's, when it names one, and the
/// parent's own recorded size otherwise.
///
/// The envelope's `budget` is required on a `@task_starting` operation, so there is always
/// one to read; `bytes` inside it is optional, and its absence means the caller stated no
/// ceiling of its own rather than a ceiling of zero.
///
/// The fallback reads a *measurement* as a ceiling, and that is deliberate rather than
/// left over. Since RFC 0028 correction 17 a pack's `content_budget.bytes` is what that pack
/// spent, so the default this resolves to is "an expansion may not cost more than the pack
/// it expands, unless the caller asks for more" — a bound a deployment can reason about,
/// which a requested ceiling copied down a family could not be. Nothing about the *child's*
/// own recorded value comes from here: what a child records is its own measurement
/// (`ChildPack::to_json`), and this number only decides what it is admitted against.
fn ceiling(record: &ContextPackRecord, envelope: &RequestEnvelope) -> Result<u64, PackError> {
    match output::Ceiling::of_budget(envelope).stated() {
        Some(bytes) => Ok(bytes),
        None => pack::budget_bytes_of(record.document()),
    }
}

/// The snapshot test an expansion runs, and the reason it is one test rather than three.
///
/// `task.resume` runs three (RFC 0026's resume table): the envelope must not name a
/// *different* snapshot than the continuation pinned, and the pinned snapshot must still be
/// held, sealed, and current in its lineage. Only the first applies to an expansion, and the
/// other two would be wrong here:
///
/// > Packs are immutable. A pack is a plan §4.4 content-addressed artifact. […]
/// > `Incompatible` means the pack is readable as history and MUST NOT support a claim.
/// >
/// > — RFC 0028, "Versioning and revision"
///
/// A continuation is *resumed* — it runs more work against the world it pinned, so that
/// world must still be current. A pack is *read*: it is a published artifact about a
/// snapshot that has already happened, and navigating it changes nothing. Refusing to
/// expand a pack because its snapshot was superseded would make exactly the history the
/// pack exists to explain unreadable, and RFC 0028's typed-outcome table has no such row.
///
/// What is a real disagreement is a caller naming one snapshot on the envelope and a pack
/// stated against another: the request is then about two different worlds, and
/// `StaleSnapshot` is the code `rule errors.common` provides "where a snapshot is named" —
/// which here it literally was.
fn stale_snapshot_check(
    envelope: &RequestEnvelope,
    snapshot: &WorkspaceHandle,
) -> Result<(), Fault> {
    if let Nullable::Value(named) = &envelope.snapshot {
        if named != snapshot {
            return Err(Fault::new(
                ErrorCode::StaleSnapshot,
                "the request names a snapshot other than the one the named pack is stated \
                 against",
            ));
        }
    }
    Ok(())
}

/// The wire `ExpansionRelation` as the pack vocabulary spells it.
///
/// An exhaustive match rather than a token lookup: the two enums are the same eleven
/// members of one closed IDL vocabulary, and a member added to either side that the other
/// does not have is a compile error here rather than a `None` at run time.
const fn crosswalk(relation: ExpansionRelation) -> PackRelation {
    match relation {
        ExpansionRelation::CausalPredecessors => PackRelation::CausalPredecessors,
        ExpansionRelation::CausalSuccessors => PackRelation::CausalSuccessors,
        ExpansionRelation::ConflictsWith => PackRelation::ConflictsWith,
        ExpansionRelation::SameOwner => PackRelation::SameOwner,
        ExpansionRelation::PropertyAutomatonStep => PackRelation::PropertyAutomatonStep,
        ExpansionRelation::ProofDependency => PackRelation::ProofDependency,
        ExpansionRelation::SourceSpan => PackRelation::SourceSpan,
        ExpansionRelation::AssumptionUses => PackRelation::AssumptionUses,
        ExpansionRelation::AbstractionOf => PackRelation::AbstractionOf,
        ExpansionRelation::RefinementOf => PackRelation::RefinementOf,
        ExpansionRelation::AlternateBranch => PackRelation::AlternateBranch,
    }
}

/// The wire `OmissionReason` as the pack vocabulary spells it — the same closed five
/// members, crosswalked exhaustively for the reason [`crosswalk`] gives.
const fn wire_reason(reason: PackReason) -> OmissionReason {
    match reason {
        PackReason::Budget => OmissionReason::Budget,
        PackReason::Redaction => OmissionReason::Redaction,
        PackReason::Unsupported => OmissionReason::Unsupported,
        PackReason::HeuristicCutoff => OmissionReason::HeuristicCutoff,
        PackReason::SliceIrrelevant => OmissionReason::SliceIrrelevant,
    }
}

/// A registered pack that turns out not to be derivable from.
///
/// Unreachable through [`ContextPackRecord::new`], which refuses such a pack at
/// registration; kept as a typed answer rather than a panic because "a daemon that
/// unwrapped a conversion would abort on a value a client chose" is the discipline this
/// crate applies everywhere else, and a registration surface is still a surface.
fn malformed_pack(_error: PackError) -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "this daemon holds no expandable form of the named pack",
    )
}

/// The wire answer to a ceiling the packer could not pack to.
///
/// One arm is a protocol outcome and the rest are not: [`BudgetError::Exhausted`] is RFC
/// 0028's budget row taking its first branch, and the three others say the registered pack
/// or its groups are not something a child can be derived from at *any* ceiling, which is
/// the same fact [`malformed_pack`] answers. The detail is a `&'static str` by `Fault`'s own
/// type, so neither carries a number a caller supplied.
fn budget_fault(error: BudgetError) -> Fault {
    match error {
        // The typed `non_resumable_reason` is the detail itself (SD-13), so a caller reading
        // the failure and a caller reading a later task record see one reason.
        BudgetError::Exhausted { .. } => Fault::exhausted(
            "the smallest conforming child of this expansion — its complete omission \
             manifest beside an empty selection — is larger than the byte budget this call \
             ran under, and a manifest is never what a budget squeezes out",
        ),
        BudgetError::Pack(_) | BudgetError::Accounting(_) | BudgetError::Manifest(_) => {
            malformed_pack(PackError::NotAPackHandle)
        }
    }
}
