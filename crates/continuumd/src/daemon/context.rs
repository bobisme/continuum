//! The `context` family: `compile` and `expand` — the wire end of PR-11's Context Pack.
//!
//! # What lands here, and what does not
//!
//! `context.expand` is served. `context.compile` reaches this family and is refused
//! `UnsupportedSemanticFeature`, for the reason [`observe`](super::observe)'s two refusals
//! give: RFC 0028's compiler is ten stages over the evidence graph, the property automaton,
//! the correspondence graph and a minimizer, and none of those exist in this workspace. An
//! operation "registered in plan §10.2 ahead of its producing subsystem […] MUST fail with
//! the typed `UnsupportedSemanticFeature` rather than degrading, guessing, or returning an
//! empty success" (`rule errors.unsupported_surface`), and a compiled-looking pack with no
//! compiler behind it is exactly the degradation that rule names.
//!
//! Expansion is a different operation, and that is why it can land while compilation
//! cannot: it is **navigation over a published pack**, not compilation from evidence.
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
//! [`DaemonState::put_context_pack`](super::state::DaemonState::put_context_pack), through
//! [`Daemon::state_mut`](super::Daemon::state_mut) — the out-of-band administration surface
//! content staging and capability provisioning already use, because "content staging and
//! capability provisioning are not operations in this protocol version (IDL §7)". Pack
//! *compilation* is not an operation this daemon serves either, so a deployment registers
//! the packs it holds the same way it registers the models `verification.start` explores.
//! When the compiler lands it will write through the same surface, and nothing in this file
//! changes.
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
//! # What is declined here, and why
//!
//! **The child pack is not published into the reference store.** A store handle is the
//! content identity of the bytes (`ContentIdentifier::identify`), and a pack's `context_id`
//! is the identity of its *question* (RFC 0027 C2, RFC 0028's idempotency rule) — two
//! spellings for one artifact unless something reconciles them, and reconciling them is a
//! decision about pack assembly rather than about expansion. Publishing under a second
//! identity would be the disagreement ADR-0013 forbids, so this bullet returns the pack on
//! the wire, names no `ArtifactRef`, and leaves the store question to the bone that owns
//! pack assembly. Nothing about the answer is weaker for it: the pack is
//! self-describing, carries its own `content_hash`, and names its parent.

use continuum_context::budget::{BudgetError, BudgetPacker};
use continuum_context::expansion::{
    Depth, ExpansionHandle, ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{
    IrretrievableReason, Manifest, OmissionReason as PackReason, OmissionRecord, Retrievability,
};
use continuum_context::pack::{self, ChildPack, PackError};
use continuum_context::selection::SelectedItem;
use continuum_intent::canonical_json::Json;
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ReferenceStore;

use std::collections::{BTreeMap, BTreeSet};

use super::Services;
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::DaemonState;
use crate::protocol::envelope::{Omission, RequestEnvelope};
use crate::protocol::operations::context::{ContextExpandRequest, ContextExpandResponse};
use crate::protocol::scalar::{ArtifactHandle, ContextHandle, Opaque, WorkspaceHandle};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{ErrorCode, ExpansionRelation, OmissionReason};

/// The `context` namespace's two operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// Held to `rule errors.common` ∪ each operation's `errors` clause by
/// `tests/daemon_context_operations.rs`, the way `observe::FAULTS` is by
/// `tests/daemon_evidence.rs`.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("context.compile", ErrorCode::UnsupportedSemanticFeature),
    ("context.expand", ErrorCode::CapabilityDenied),
    ("context.expand", ErrorCode::MalformedRequest),
    ("context.expand", ErrorCode::StaleSnapshot),
    ("context.expand", ErrorCode::UnsupportedSemanticFeature),
    ("context.expand", ErrorCode::BudgetExhausted),
];

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

/// A pack, or a group, this daemon will not register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextPackError {
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
        _services: &Services,
        _store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::ContextExpand(request) => expand(call.envelope, request, state),
            Arguments::ContextCompile(_) => Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "no Context Pack compiler is served by this daemon; a pack is compiled from \
                 the evidence graph, the property automaton and the correspondence graph, \
                 and none of those is wired here",
            )),
            // Unreachable: the dispatcher checked shape agreement before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
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
    let stated = envelope
        .budget
        .value()
        .and_then(|budget| budget.bytes.value().copied())
        .map(crate::protocol::scalar::ByteCount::bytes);
    match stated {
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
