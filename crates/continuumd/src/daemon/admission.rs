//! RFC 0027's admission predicate, T1–T4, decided before any semantic work.
//!
//! ```text
//! T1 (level)      registry_minimum(operation)  ≤  descriptor.level
//! T2 (scope)      every snapshot, intent, and artifact class the request names
//!                 lies inside the descriptor's scope
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

use continuum_workspace::artifact_path::ArtifactClass;

use super::family::ScopeClaim;
use super::state::{DaemonState, store_level};
use crate::protocol::envelope::RequestEnvelope;
use crate::protocol::handshake::{CapabilityDescriptor, CapabilityProfile};
use crate::protocol::scalar::{CapabilityHandle, Timestamp};
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
    // T4, first half: registered and unrevoked. An unregistered token and a revoked one
    // take the same path, which is what makes them indistinguishable.
    let grant = state.grant(&envelope.capability).ok_or(Denied)?;
    let descriptor = &grant.descriptor;

    // T4: the actor binds to the capability. "A mismatch is `CapabilityDenied`, never
    // `MalformedRequest`: an attempt to act as another principal is an authorization
    // failure and MUST NOT be distinguishable from any other one."
    if descriptor.actor != envelope.actor {
        return Err(Denied);
    }

    // T4: unexpired. Time is not ambient (INV-005, ADR-0003), so a deployment that supplies
    // no reading cannot judge an expiring capability and the predicate fails closed — "a
    // daemon that cannot decide admission fails closed" (RFC 0027).
    if let Some(expiry) = descriptor.expires_at.value() {
        match now {
            Some(reading) if reading < expiry => {}
            _ => return Err(Denied),
        }
    }

    // T4: the presented capability is the connection's, or a descendant that narrows.
    if &descriptor.capability != connection {
        admissible_on_connection(state, &descriptor.capability, connection)?;
    }

    // T1: the ladder, read off the registry.
    if store_level(descriptor.level) < store_level(spec.authority) {
        return Err(Denied);
    }

    // T2: instance scope for snapshots and intents, class scope for everything else. An
    // empty list means unrestricted *within the level*.
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

/// Walk the delegation chain from `presented` to `connection`, requiring every hop to
/// narrow.
fn admissible_on_connection(
    state: &DaemonState,
    presented: &CapabilityHandle,
    connection: &CapabilityHandle,
) -> Result<(), Denied> {
    // A delegation chain is finite and acyclic by construction — `register_capability`
    // records a parent that already exists — but the bound is stated rather than assumed,
    // because a daemon does not loop on a value a deployment chose.
    let mut child = presented.clone();
    for _ in 0..MAX_DELEGATION_HOPS {
        let grant = state.grant(&child).ok_or(Denied)?;
        let parent_handle = grant.parent.clone().ok_or(Denied)?;
        let parent = state.grant(&parent_handle).ok_or(Denied)?;
        if !narrows(&grant.descriptor, &parent.descriptor) {
            return Err(Denied);
        }
        if parent_handle == *connection {
            return Ok(());
        }
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
    {
        return false;
    }
    profile_narrows(child.profile.value(), parent.profile.value())
}

/// `rule capability.profile_narrowing`'s four field rules.
fn profile_narrows(child: Option<&CapabilityProfile>, parent: Option<&CapabilityProfile>) -> bool {
    let Some(child) = child else {
        // No profile is the fail-closed reading: it grants nothing, so it narrows anything.
        return true;
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
fn scope_narrows<T: PartialEq>(narrow: &[T], wide: &[T]) -> bool {
    wide.is_empty() || contains_all(wide, narrow)
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
