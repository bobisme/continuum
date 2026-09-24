//! RFC 0027's admission predicate, T1–T4, decided before any semantic work.
//!
//! ```text
//! T1 (level)      registry_minimum(operation)  ≤  descriptor.level
//! T2 (scope)      every snapshot, intent, and artifact class the request names
//!                 lies inside the descriptor's scope, and so does every other
//!                 instance it names, class by class (3.7, `instances`)
//! T3 (privilege)  the operation is not @privileged, or the profile explicitly
//!                 grants it — and no profile denies it
//! T4 (validity)   the capability is registered, unexpired, unrevoked, its actor
//!                 matches the request's, and it is the connection's own capability
//!                 or a descendant of it
//! ```
//!
//! Four properties of this module are load-bearing and each is a property of its *types*
//! rather than of its discipline:
//!
//! - **It can only deny.** [`admit`] returns the descriptor it was given or [`Denied`];
//!   there is no path on which a test widens anything. "The ladder is an upper bound. Scope,
//!   privilege, and validity narrow it; nothing widens it" (RFC 0027 A/24).
//! - **A denial carries nothing.** [`Denied`] is a unit struct, so `CapabilityDenied` "is
//!   the single answer to every admission failure" (X1) is unrepresentable otherwise: level,
//!   scope, privilege, expiry, revocation, actor mismatch, and a non-descendant capability
//!   all produce one value with zero bits in it.
//! - **It sees no store.** The signature takes [`DaemonState`] only to look a capability up
//!   *by the caller's own token*, which "reveals nothing about any artifact"
//!   (`publication.rs`); no index, no snapshot, and no intent is consulted, so X3 —
//!   "denial precedes semantic work and precedes the index" — holds structurally.
//! - **T1 is read off the registry.** `spec.authority` is the IDL's own `authority` clause,
//!   which the conformance suite already holds equal to RFC 0027's registry table, so the
//!   ladder is never restated here.
//!
//! # The order the levels are compared in
//!
//! The wire `AuthorityLevel` is a closed enum with no [`Ord`]: its declaration order is
//! the ladder, but the protocol layer deliberately derives no ordering, because an ordering
//! is a *meaning* and the IDL's enum is a vocabulary. The meaning lives in the landed
//! store-side enum, "whose declaration order *is* the ladder and whose `Ord` is 'at least
//! this level'" (RFC 0027, "Landed vocabulary"), so the comparison goes through
//! [`store_level`] and there is exactly one ladder in the workspace.

use std::collections::{BTreeMap, BTreeSet};

use continuum_workspace::artifact_path::ArtifactClass;

use super::family::ScopeClaim;
use super::provisioning::class_of;
use super::state::{DaemonState, store_level};
use crate::protocol::envelope::RequestEnvelope;
use crate::protocol::handshake::{CapabilityDescriptor, CapabilityProfile};
use crate::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, IntentHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::spec::{Annotation, OperationSpec, Optional};
use crate::protocol::vocabulary::DataGrant;

/// The whole of what a refused caller learns.
///
/// > `CapabilityDenied` is the single answer to every admission failure. Level, scope,
/// > privilege, expiry, revocation, actor mismatch, and a non-descendant capability all
/// > produce it, and a client MUST NOT be able to tell them apart.
/// >
/// > — RFC 0027 X1
///
/// A unit struct has exactly one value, so the rule is not one this module follows — it is
/// one its return type cannot break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Denied;

/// The additional grant an operation requires beyond its authority level.
///
/// The IDL declares no per-operation `data_grants` clause, so this is the one pair RFC 0027
/// fixes, stated once:
///
/// > `data_grants` carries the requirements that are neither level nor privilege:
/// > `observe.ingest` requires `production_trace` in addition to `execute` (R-4), and a
/// > capability without it is denied even though its level admits the operation.
/// >
/// > — RFC 0027, "The admission predicate"
///
/// It lives here rather than in the `observe` family because T3 is decided before any
/// family runs, and because the sibling that lands `observe` should inherit the check
/// rather than write it.
#[must_use]
pub fn required_grant(operation: &str) -> Option<DataGrant> {
    match operation {
        "observe.ingest" => Some(DataGrant::ProductionTrace),
        _ => None,
    }
}

/// Whether `operation` acts on the deployment as a whole, and so requires an unscoped
/// grant (protocol 3.8, `rule signing.identities`).
///
/// A signer identity is not an artifact of any plan §4.4 class and has no handle a scope
/// list could name: the `signing` operations mint, rotate, and revoke the deployment's own
/// identities, sign as the deployment, and read its registry, which names every signer and
/// the actor of every audit record. The two bundle operations sign as the deployment and
/// carry that registry, or adopt records into it. None of that is inside a part of the
/// deployment, so a grant scoped to a part — any non-empty `snapshots`, `intents`,
/// `artifact_classes`, or `instances` list — does not reach it. The test runs on the
/// presenting grant; the delegation chain above it already narrows (D4), so an unscoped
/// grant stands only under unscoped parents.
#[must_use]
pub fn requires_unscoped_grant(operation: &str) -> bool {
    operation.starts_with("signing.")
        || matches!(operation, "intent.export_bundle" | "intent.import_bundle")
}

/// Whether `descriptor` carries no scope restriction at all: every scope list is empty,
/// which RFC 0027 reads as "unrestricted within the level".
#[must_use]
pub fn is_unscoped(descriptor: &CapabilityDescriptor) -> bool {
    descriptor.snapshots.is_empty()
        && descriptor.intents.is_empty()
        && descriptor.artifact_classes.is_empty()
        && descriptor
            .instances
            .value()
            .is_none_or(|instances| instances.is_empty())
}

/// Decide T1–T4 for one request.
///
/// `connection` is the capability the handshake settled on; the request may present a
/// *narrower* one, and RFC 0027 defines narrower as "the connection's own capability or a
/// descendant of it in the delegation tree" whose admission set is a subset — checked here
/// by walking the delegation chain and requiring each hop to narrow on every field
/// (`rule capability.profile_narrowing`, D6/D7).
///
/// # Errors
///
/// [`Denied`], carrying nothing.
pub fn admit<'state>(
    spec: &OperationSpec,
    envelope: &RequestEnvelope,
    claim: &ScopeClaim,
    state: &'state DaemonState,
    connection: &CapabilityHandle,
    now: Option<&Timestamp>,
) -> Result<&'state CapabilityDescriptor, Denied> {
    let descriptor = standing(
        state,
        &envelope.capability,
        &envelope.actor,
        connection,
        now,
    )?;

    // T1: the ladder, read off the registry.
    if store_level(descriptor.level) < store_level(spec.authority) {
        return Err(Denied);
    }

    // T2: instance scope for snapshots and intents, class scope for everything else, and
    // (3.7) instance scope for any other class `instances` lists. An empty list means
    // unrestricted *within the level*.
    let named_snapshots = envelope
        .snapshot
        .value()
        .into_iter()
        .chain(&claim.snapshots);
    if !descriptor.snapshots.is_empty() {
        for snapshot in named_snapshots {
            if !descriptor.snapshots.contains(snapshot) {
                return Err(Denied);
            }
        }
    }
    let named_intents = envelope.intent.value().into_iter().chain(&claim.intents);
    if !descriptor.intents.is_empty() {
        for intent in named_intents {
            if !descriptor.intents.contains(intent) {
                return Err(Denied);
            }
        }
    }
    if !descriptor.artifact_classes.is_empty() {
        for class in &claim.classes {
            if !descriptor
                .artifact_classes
                .iter()
                .any(|held| held == *class)
            {
                return Err(Denied);
            }
        }
    }

    // T2, 3.7: every other instance the request names, decided per class. A class none of
    // whose instances is listed keeps the class scope tested above; a class with a listed
    // instance admits only listed instances (`rule capability.instance_scope`). The test is
    // on the handle's spelling alone, so it neither reads the store nor depends on who was
    // told about the handle (X3, G1-07).
    //
    // Each named instance is bound to its own class first: its prefix must name a class,
    // and that class must be one the claim declares, so the class half above has decided
    // it. A handle of no class, or of a class the family did not claim, is refused rather
    // than decided by the wrong class (class confusion, cr-3hcpn4).
    let claimed_class =
        |named: &str| class_of(named).is_some_and(|class| claim.classes.contains(&class.token()));
    let scope = InstanceScope::of(descriptor);
    if !claim
        .instances
        .iter()
        .all(|named| claimed_class(named) && scope.admits(named))
    {
        return Err(Denied);
    }

    // T2, 3.8: an operation on the deployment as a whole needs a grant scoped to no part of
    // it (`rule signing.identities`).
    if requires_unscoped_grant(spec.name) && !is_unscoped(descriptor) {
        return Err(Denied);
    }

    // T3: privilege, plus the per-operation restriction a total order cannot express.
    let profile = descriptor.profile.value();
    if denies_operation(profile, spec.name) {
        return Err(Denied);
    }
    if spec.has(Annotation::Privileged) && !grants_operation(profile, spec.name) {
        return Err(Denied);
    }
    if let Some(grant) = required_grant(spec.name) {
        match profile {
            Some(profile) if profile.data_grants.contains(&grant) => {}
            _ => return Err(Denied),
        }
    }

    Ok(descriptor)
}

/// T4 alone: whether `presented` still stands for `actor` on the connection that settled on
/// `connection`, at the time reading `now`.
///
/// Registered and unrevoked, bound to `actor`, unexpired, and either the connection's own
/// capability or a descendant whose every hop still exists and still narrows. A revoked
/// parent revokes its children (D8) because the walk needs every hop, and a parent
/// re-provisioned narrower than its child refuses the child because the walk re-checks
/// each hop. [`admit`] decides this first for every request, and subscription delivery
/// decides it again before every pass, because a subscription outlives the request that
/// opened it (RFC 0027 R1, `rule capability.instance_scope`).
///
/// # Errors
///
/// [`Denied`], carrying nothing: every T4 failure is the one denial (X1).
pub fn standing<'state>(
    state: &'state DaemonState,
    presented: &CapabilityHandle,
    actor: &ActorId,
    connection: &CapabilityHandle,
    now: Option<&Timestamp>,
) -> Result<&'state CapabilityDescriptor, Denied> {
    // Registered and unrevoked. An unregistered token and a revoked one take the same
    // path, which is what makes them indistinguishable.
    let grant = state.grant(presented).ok_or(Denied)?;
    let descriptor = &grant.descriptor;

    // The actor binds to the capability. "A mismatch is `CapabilityDenied`, never
    // `MalformedRequest`: an attempt to act as another principal is an authorization
    // failure and MUST NOT be distinguishable from any other one."
    if descriptor.actor != *actor {
        return Err(Denied);
    }

    // Unexpired. Time is not ambient (INV-005, ADR-0003), so a deployment that supplies no
    // reading cannot judge an expiring capability and the predicate fails closed — "a
    // daemon that cannot decide admission fails closed" (RFC 0027). Each hop's expiry is
    // no later than its parent's (D5, [`narrows`]), so the leaf's reading covers the chain.
    if let Some(expiry) = descriptor.expires_at.value() {
        match now {
            Some(reading) if reading < expiry => {}
            _ => return Err(Denied),
        }
    }

    // The presented capability is the connection's, or a descendant of it, and its whole
    // stored chain stands up to a root — also when it *is* the connection's capability,
    // because a delegated grant can open a connection of its own and must not outlive its
    // parent there either (D8, R1; cr-3hcpn4).
    chain_stands(state, &descriptor.capability, connection)?;
    Ok(descriptor)
}

/// A handle a handler reached from one the request named, by a lookup admission may not
/// make (X3): a continuation's task and snapshot, a task's snapshot and continuation, a
/// pack's snapshot, and so on.
#[derive(Debug, Clone, Copy)]
pub enum Derived<'handle> {
    /// A `ws_*` snapshot, decided by the `snapshots` list as T2 decides a named one.
    Snapshot(&'handle WorkspaceHandle),
    /// An `in_*` intent, decided by the `intents` list as T2 decides a named one.
    Intent(&'handle IntentHandle),
    /// Any other instance, decided by `artifact_classes` and `instances` as T2 decides a
    /// named one. A spelling of no class is never admitted.
    Instance(&'handle str),
}

/// A [`Derived`] handle a call consulted, owned so the idempotency ledger can keep it.
///
/// A replay returns a recorded outcome without running the handler, so the handler's own
/// derived-handle decisions do not run again. The ledger keeps every derived handle the
/// original call admitted, and a replay re-decides each one against the presenting grant
/// (cr-3lrkq3). Recorded by [`Call::admits`](super::family::Call::admits), the one place a
/// handler decides a derived handle, so every operation is covered by the same mechanism.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Consulted {
    /// A `ws_*` snapshot.
    Snapshot(WorkspaceHandle),
    /// An `in_*` intent.
    Intent(IntentHandle),
    /// Any other instance.
    Instance(String),
}

impl Consulted {
    /// The owned form of `derived`.
    #[must_use]
    pub fn of(derived: Derived<'_>) -> Self {
        match derived {
            Derived::Snapshot(handle) => Self::Snapshot(handle.clone()),
            Derived::Intent(handle) => Self::Intent(handle.clone()),
            Derived::Instance(handle) => Self::Instance(handle.to_owned()),
        }
    }

    /// The borrowed form, for [`admits_derived`].
    #[must_use]
    pub fn as_derived(&self) -> Derived<'_> {
        match self {
            Self::Snapshot(handle) => Derived::Snapshot(handle),
            Self::Intent(handle) => Derived::Intent(handle),
            Self::Instance(handle) => Derived::Instance(handle),
        }
    }
}

/// Whether `descriptor` admits a derived handle by the same T2 it applies to a named one
/// (`rule capability.instance_scope`, the derived-handle clause; cr-3hcpn4).
///
/// Admission decides what a request names. A handler that resolves a named handle into an
/// instance of another class — and then reads it, writes it, or reports anything about
/// it — decides that instance here, *before* the read, the write, or the answer, and
/// refuses with the one `CapabilityDenied` when it is out of scope. Scope is the capability's
/// property, never derived from what the caller named (`rule
/// authorization.independent_of_handles`).
#[must_use]
pub fn admits_derived(descriptor: &CapabilityDescriptor, derived: Derived<'_>) -> bool {
    match derived {
        // A snapshot or intent a handler derived is decided as a named one of its class
        // would be: its list, and the class held (cr-3hcpn4). The same handle then gets the
        // same answer whether it arrives typed or spelled as a generic handle.
        Derived::Snapshot(snapshot) => {
            class_held(descriptor, ArtifactClass::WorkspaceSnapshot)
                && (descriptor.snapshots.is_empty() || descriptor.snapshots.contains(snapshot))
        }
        Derived::Intent(intent) => {
            class_held(descriptor, ArtifactClass::IntentContract)
                && (descriptor.intents.is_empty() || descriptor.intents.contains(intent))
        }
        Derived::Instance(named) => {
            let Some(class) = class_of(named) else {
                return false;
            };
            let class_held = class_held(descriptor, class);
            // A `ws_` or `in_` spelled as a generic handle is decided by its own list, as
            // the two typed arms above decide it; every other class by `instances`.
            let listed = match class {
                ArtifactClass::WorkspaceSnapshot => {
                    descriptor.snapshots.is_empty()
                        || descriptor
                            .snapshots
                            .iter()
                            .any(|held| held.as_str() == named)
                }
                ArtifactClass::IntentContract => {
                    descriptor.intents.is_empty()
                        || descriptor.intents.iter().any(|held| held.as_str() == named)
                }
                _ => InstanceScope::of(descriptor).admits(named),
            };
            class_held && listed
        }
    }
}

/// Whether `descriptor`'s `artifact_classes` holds `class`; an empty list holds every class.
fn class_held(descriptor: &CapabilityDescriptor, class: ArtifactClass) -> bool {
    descriptor.artifact_classes.is_empty()
        || descriptor
            .artifact_classes
            .iter()
            .any(|held| held == class.token())
}

/// Whether `wide` admits everything `narrow` admits, on every field T1–T3 decide: the level,
/// the snapshot, intent, and class scopes, the instance scope class by class, and the
/// profile. Standing (T4) is not compared; it is decided for the presenting capability on
/// its own.
///
/// Used by a replay: an outcome recorded under `narrow` names only what `narrow` admitted,
/// so a presenting grant that covers it admits all of it by monotonicity (cr-3hcpn4).
#[must_use]
pub fn covers(wide: &CapabilityDescriptor, narrow: &CapabilityDescriptor) -> bool {
    store_level(narrow.level) <= store_level(wide.level)
        && scope_narrows(&narrow.snapshots, &wide.snapshots)
        && scope_narrows(&narrow.intents, &wide.intents)
        && scope_narrows(&narrow.artifact_classes, &wide.artifact_classes)
        && instances_narrow(narrow.instances.value(), wide.instances.value())
        && profile_narrows(narrow.profile.value(), wide.profile.value())
}

/// Whether `descriptor`'s instance scope admits the handle spelled `named`.
///
/// > For each class, the scope is decided by that class alone: a class none of whose
/// > instances is listed keeps the scope `artifact_classes` gives it, and a class with at
/// > least one listed instance admits a request only when every instance of that class the
/// > request names is listed.
/// >
/// > — `rule capability.instance_scope`
///
/// This is only the instance half of T2. The class half is decided from
/// `ScopeClaim::classes`, so a handle of a class the capability does not hold is refused
/// there and is answered `true` here. A handle of no class is answered `false`: nothing can
/// have decided its class, so it fails closed. Exposed because an answer that *selects* artifacts
/// the request did not name — `evidence.query`, `evidence.subscribe` — is bounded by the
/// same predicate after admission (C1), and a second spelling of it could drift.
#[must_use]
pub fn admits_instance(descriptor: &CapabilityDescriptor, named: &str) -> bool {
    InstanceScope::of(descriptor).admits(named)
}

/// A descriptor's `instances`, indexed by class once, so that deciding many handles
/// against it costs one logarithmic lookup each rather than a scan of the list per handle.
///
/// Admission decides every instance a request names against one index, and a selection
/// (`evidence.query`, the `evidence.subscribe` frontier) filters every handle it selects
/// against one index. [`admits_instance`] is the one-handle spelling of the same test.
pub struct InstanceScope<'grant> {
    /// `None` when `instances` is absent: every class keeps its class scope.
    listed: Option<BTreeMap<ArtifactClass, BTreeSet<&'grant str>>>,
}

impl<'grant> InstanceScope<'grant> {
    /// Index `descriptor`'s instance scope by class.
    #[must_use]
    pub fn of(descriptor: &'grant CapabilityDescriptor) -> Self {
        Self {
            listed: descriptor.instances.value().map(|listed| by_class(listed)),
        }
    }

    /// Whether this scope admits the handle spelled `named`
    /// (`rule capability.instance_scope`).
    ///
    /// A spelling of no class is never admitted, whatever the scope lists: no class scope
    /// can have decided it, so the answer fails closed.
    #[must_use]
    pub fn admits(&self, named: &str) -> bool {
        let Some(class) = class_of(named) else {
            return false;
        };
        let Some(listed) = &self.listed else {
            return true;
        };
        listed.get(&class).is_none_or(|held| held.contains(named))
    }
}

/// `listed`, grouped by the class each member's prefix names. A member of no class is left
/// out: provisioning refuses one, so none reaches a registered descriptor.
fn by_class(listed: &[ArtifactHandle]) -> BTreeMap<ArtifactClass, BTreeSet<&str>> {
    let mut index: BTreeMap<ArtifactClass, BTreeSet<&str>> = BTreeMap::new();
    for held in listed {
        if let Some(class) = class_of(held.as_str()) {
            index.entry(class).or_default().insert(held.as_str());
        }
    }
    index
}

/// The class prefix a snapshot or intent handle belongs to, as `ScopeClaim` spells it.
///
/// Exposed so a family declaring a [`ScopeClaim`] names the class through the store's own
/// class table rather than through a literal that could drift from it.
#[must_use]
pub const fn class_token(class: ArtifactClass) -> &'static str {
    class.token()
}

/// Whether an absent-or-present profile explicitly grants a `@privileged` operation.
///
/// An absent profile is the fail-closed reading and is "exactly equivalent to one whose
/// lists are empty and whose `cross_principal_sharing` is false; a daemon MUST NOT read
/// absence as 'unrestricted'" (`rule capability.profile_narrowing`).
fn grants_operation(profile: Option<&CapabilityProfile>, operation: &str) -> bool {
    profile.is_some_and(|profile| {
        profile
            .privileged_operations
            .iter()
            .any(|name| name.as_str() == operation)
    })
}

/// Whether a profile denies an operation outright. "An operation named both here and in
/// `privileged_operations` is denied", which is why this is checked first.
fn denies_operation(profile: Option<&CapabilityProfile>, operation: &str) -> bool {
    profile.is_some_and(|profile| {
        profile
            .denied_operations
            .iter()
            .any(|name| name.as_str() == operation)
    })
}

/// Walk the stored delegation chain from `presented` to its root, requiring every parent
/// to exist and every hop to narrow, and requiring `connection` to be `presented` itself or
/// one of its ancestors.
///
/// The walk goes to the root rather than stopping at the connection's capability, because
/// a connection may itself be held under a delegated grant: revoking, narrowing, or letting
/// expire any ancestor of it must end that connection's authority as well (D8, R1). A grant
/// with no parent is a root and stands on its own registration.
fn chain_stands(
    state: &DaemonState,
    presented: &CapabilityHandle,
    connection: &CapabilityHandle,
) -> Result<(), Denied> {
    // A delegation chain is finite and acyclic by construction — `register_capability`
    // records a parent that already exists — but the bound is stated rather than assumed,
    // because a daemon does not loop on a value a deployment chose.
    let mut on_connection = presented == connection;
    let mut child = presented.clone();
    for _ in 0..MAX_DELEGATION_HOPS {
        let grant = state.grant(&child).ok_or(Denied)?;
        let Some(parent_handle) = grant.parent.clone() else {
            return if on_connection { Ok(()) } else { Err(Denied) };
        };
        let parent = state.grant(&parent_handle).ok_or(Denied)?;
        if !narrows(&grant.descriptor, &parent.descriptor) {
            return Err(Denied);
        }
        on_connection |= parent_handle == *connection;
        child = parent_handle;
    }
    Err(Denied)
}

/// The longest delegation chain admission will walk.
///
/// `delegation_depth` already bounds it — each hop strictly decreases it — so this is the
/// second bound rather than the first, and exists so that a mis-provisioned registry costs
/// a denial instead of a hang.
const MAX_DELEGATION_HOPS: usize = 64;

/// Whether `child`'s admission set is a subset of `parent`'s, field by field.
///
/// This is RFC 0027 D3–D6 as a computation. D7 — "a child's admission set MUST be a subset
/// of its parent's" — is the composition of these, which is what makes it checkable rather
/// than asserted:
///
/// > Before 3.1 it was checkable for three of the four tests and asserted for the fourth,
/// > because T3 had no wire representation to compare; that was F1's practical cost, and
/// > paying it is what makes D7 a computation.
fn narrows(child: &CapabilityDescriptor, parent: &CapabilityDescriptor) -> bool {
    if store_level(child.level) > store_level(parent.level) {
        return false;
    }
    if parent.delegation_depth == 0 || child.delegation_depth >= parent.delegation_depth {
        return false;
    }
    if let Some(parent_expiry) = parent.expires_at.value() {
        // A child of an expiring parent expires, and never later than its parent.
        match child.expires_at.value() {
            Some(child_expiry) if child_expiry <= parent_expiry => {}
            _ => return false,
        }
    }
    if !scope_narrows(&child.snapshots, &parent.snapshots)
        || !scope_narrows(&child.intents, &parent.intents)
        || !scope_narrows(&child.artifact_classes, &parent.artifact_classes)
        || !instances_narrow(child.instances.value(), parent.instances.value())
    {
        return false;
    }
    profile_narrows(child.profile.value(), parent.profile.value())
}

/// `rule capability.instance_scope`'s delegation clause: class by class, a child
/// instance-scopes every class its parent instance-scopes, to a subset.
///
/// A class the parent leaves at class scope is unconstrained here, because any child list
/// for it is a narrowing, and the class half of D4 is `artifact_classes`'s own test.
fn instances_narrow(
    child: Option<&Vec<ArtifactHandle>>,
    parent: Option<&Vec<ArtifactHandle>>,
) -> bool {
    let Some(parent) = parent else {
        return true;
    };
    let parent = by_class(parent);
    let child = by_class(child.map_or(&[], Vec::as_slice));
    // The parent scopes each of its classes to instances, so the child must too, and every
    // child instance of the class must be one the parent lists.
    parent
        .iter()
        .all(|(class, theirs)| child.get(class).is_some_and(|mine| mine.is_subset(theirs)))
}

/// `rule capability.profile_narrowing`'s four field rules.
fn profile_narrows(child: Option<&CapabilityProfile>, parent: Option<&CapabilityProfile>) -> bool {
    let Some(child) = child else {
        // No profile grants nothing, and it also denies nothing. So it narrows a parent only
        // when that parent denies nothing either: a parent's deny list binds every
        // descendant, and a child without a profile is not a way out of it (cr-3hcpn4).
        return parent.is_none_or(|parent| parent.denied_operations.is_empty());
    };
    let Some(parent) = parent else {
        // "A child of a parent with no profile MUST have no profile grants either."
        return child.privileged_operations.is_empty()
            && child.data_grants.is_empty()
            && !child.cross_principal_sharing;
    };
    // A profile list is a plain set, not a scope list: an empty `privileged_operations`
    // grants nothing rather than everything, and an empty `denied_operations` denies
    // nothing rather than everything. Reading either as "unrestricted" would invert the
    // fail-closed direction `rule capability.profile_narrowing` fixes, so these are subset
    // and superset tests with no empty-list convention on top.
    contains_all(&parent.privileged_operations, &child.privileged_operations)
        && contains_all(&parent.data_grants, &child.data_grants)
        && contains_all(&child.denied_operations, &parent.denied_operations)
        && (!child.cross_principal_sharing || parent.cross_principal_sharing)
}

/// Whether `narrow` is a narrowing of the *scope* list `wide`, where an empty list means
/// "unrestricted within the level" — the reading the IDL fixes for every scope list.
///
/// An empty list is the *widest* scope ("unrestricted within the level"), so an empty
/// child list narrows only an empty parent list. Before cr-3hcpn4 an empty child list was
/// read as the empty set and passed under any parent, which let a delegate with no scope
/// list escape a scoped parent (D4).
fn scope_narrows<T: PartialEq>(narrow: &[T], wide: &[T]) -> bool {
    wide.is_empty() || (!narrow.is_empty() && contains_all(wide, narrow))
}

/// Whether every member of `narrow` appears in `wide`.
fn contains_all<T: PartialEq>(wide: &[T], narrow: &[T]) -> bool {
    narrow.iter().all(|item| wide.contains(item))
}

/// Whether the descriptor's optional profile permits cross-principal deduplication.
///
/// Read by the publication path: RFC 0026 makes the sharing policy "a capability property
/// rather than a daemon-global flag, because a global flag cannot be scoped, delegated, or
/// revoked". Absent is false.
#[must_use]
pub fn cross_principal_sharing(descriptor: &CapabilityDescriptor) -> bool {
    match &descriptor.profile {
        Optional::Present(profile) => profile.cross_principal_sharing,
        Optional::Absent => false,
    }
}
