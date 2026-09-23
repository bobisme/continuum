//! Atomic artifact publication and possession-independent authorization
//! (plan §4.4, §4.5, PR 2 / IMPL-05 and IMPL-06).
//!
//! # What this module decides
//!
//! Two rules from plan §4.4–§4.5, expressed as types rather than as discipline.
//!
//! > **INV-017 — Semantic atomicity of publication.** Artifacts become visible only
//! > after their content, provenance, and references are durably committed.
//! >
//! > — `notes/plan/plan.md` §2
//!
//! > Handles carry a kind prefix but are otherwise structureless. Content-addressed
//! > identities are derivable by anyone holding the content — the §4.5 existence-oracle
//! > rule exists because of this — so handles are identifiers, not secrets, and confer
//! > no authority. Capability tokens (`cap_*`) are minted randomly and do confer
//! > authority. Authorization is always checked independently of handle possession.
//! >
//! > — `notes/plan/plan.md` §4.4
//!
//! The deliverable is the publication state machine plus [`ReferenceStore`], an
//! in-memory store that implements it exactly. It is the semantic reference the
//! G0-DX-13 pass condition — "one semantic identity; no lost receipts; deterministic
//! result" (`notes/plan/notes/G0_SPIKE_MATRIX.md`) — runs against.
//!
//! # Why this module lives in `continuum-workspace`
//!
//! This crate's responsibility statement already carries the store-side half of plan
//! §4.4: [`artifact_path`](crate::artifact_path) owns "the *spelling* of a handle and
//! the *place* the artifact with that handle is stored", because this crate is the top
//! of the dependency islands (`START_HERE_IMPLEMENTATION.md`) and so `continuumd`, the
//! evidence graph, and the task layer can agree on one store layout without importing
//! each other.
//!
//! *When* an artifact becomes visible at that place, and *who* may look, are the same
//! question one level up, and they need the same single owner for the same reason: plan
//! §11.7 makes the evidence graph's write model "per-artifact atomic (INV-017)", docs/35
//! makes publication ordering a daemon obligation, and RFC 0030 §"Atomicity" repeats it
//! for the incremental query index. Three consumers, one rule; if each spelled it
//! locally they would drift, and the only crate all three may import is this one.
//!
//! The division of labor is unchanged from [`artifact_path`](crate::artifact_path):
//!
//! - **identity** is `continuum-value`'s (PR 2, ADR-0013). This module never computes
//!   one: it calls a [`ContentIdentifier`] and treats the result as opaque;
//! - **durability** is `continuumd`'s (PR 6+). [`ReferenceStore`] is in memory. It fixes
//!   the semantics — the order of commits, what a crash may leave behind, what a refusal
//!   may reveal — that a real store must then implement over a real disk;
//! - **the wire** is `continuumd`'s (PR 5). [`CapabilityDenied`] and
//!   [`PublicationAborted`] here are the store-side spellings of RFC 0026's error codes
//!   of the same name, not the protocol envelope.
//!
//! No new dependency edge was needed: identity arrives through a trait, so this module
//! adds nothing to the crate's dependency closure and `continuum-workspace` remains the
//! stdlib-only island top.
//!
//! # The publication state machine
//!
//! ```text
//!                     stage                commit_content            commit_index
//!   (class, content) ───────▶ Staged ──────────────────▶ ContentCommitted ─────────▶ Published
//!                       │        │                            │                        │
//!                       │        │ abandon / drop             │ abandon / drop         │
//!                       ▼        ▼                            ▼                        ▼
//!                  CapabilityDenied ◀── PublicationAborted ──▶ PublicationAborted    receipt
//!                                          (nothing stored)      (unreachable content,
//!                                                                 GC-eligible)
//! ```
//!
//! Each transition consumes its predecessor by value, so the states are linear: a staged
//! publication can be committed once, and [`CommittedContent::commit_index`] cannot be
//! reached except through [`StagedPublication::commit_content`].
//!
//! > Publication is ordered: content MUST be committed before the index entry that names
//! > it. A crash between the two therefore leaves unreachable content, which is
//! > garbage-collectable, and never a stale index entry pointing at content that was
//! > never written. The asymmetry is deliberate — wasted bytes are recoverable, a
//! > dangling index entry is a lie about what the store holds.
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Crash safety and the index
//! >   verifier" (SD-09, absorbing plan §4.5)
//!
//! That ordering is an obligation on every implementation, so it is encoded in the API
//! shape rather than left to a comment: [`CommittedContent`] is the *witness* that
//! content is durable, it is returned by nothing but the content commit, and the index
//! commit is a method on it. Writing the index first is not a bug an implementer can
//! make here — it is a program that does not compile.
//!
//! Dropping either intermediate state is an abort, which is how a crash is modeled:
//! dropping a [`StagedPublication`] leaves the store untouched, and dropping a
//! [`CommittedContent`] leaves exactly the crash residue docs/35 describes — unreachable
//! content that [`StoreAudit::fsck`] classifies and [`ReferenceStore::collect_garbage`]
//! reclaims. Neither leaves an index entry, and neither yields a receipt.
//!
//! docs/35's publication pipeline has seven steps (validate schema and size; verify
//! referenced inputs; invoke the required independent checker; compute content identity;
//! commit artifact; commit evidence-graph edges and status; emit the subscription
//! update). This module is steps 4–6. Steps 1–3 need the schema and checker layers and
//! step 7 needs the daemon's subscription plumbing; both are later PRs, and neither
//! changes the ordering above.
//!
//! # Convergence: first publisher wins, nobody loses a receipt
//!
//! Publication is keyed by identity. The first publisher of a given identity creates the
//! index entry; a later publisher of byte-identical content converges onto *that* entry
//! rather than creating a second one, and both publishers get a receipt. The receipt
//! ledger is append-only (plan §18.5), so N concurrent publishers of identical content
//! produce one identity and N receipts — the G0-DX-13 pass condition.
//!
//! Convergence is *not* a silent merge. If the computed identity already names content
//! that is not byte-identical, the publication aborts with
//! [`AbortReason::IdentityCollision`] rather than conflating two artifacts:
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-canonical-state-identity.md`
//!
//! Exact comparison is the store's half of ADR-0013 and it is implemented here. Choosing
//! the identity function, and resolving a collision into two distinct identities in a
//! certified lane, are `continuum-value`'s (PR 2 / IMPL-03, IMPL-04).
//!
//! # Possession-independent authorization
//!
//! > Possession of an artifact handle never implies authorization. Handles are
//! > content-derived and therefore derivable by anyone holding the content, so
//! > authorization MUST be checked below the adapter, independently of handle possession
//! > (plan §4.4, ADR-0037).
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`
//!
//! An [`ArtifactHandle`] is a name. It has no read method, it cannot be converted into a
//! [`CapabilityToken`] (the conversion rejects every class but `cap_*`, which is the one
//! class no content-addressed handle can have), and every store operation takes a token
//! *beside* the handle. A leaked handle therefore buys nothing.
//!
//! The refusal is where this is made airtight. [`CapabilityDenied`] is a unit struct: it
//! has exactly one value and therefore carries zero bits. [`ReferenceStore::read`]
//! returns `Result<Vec<u8>, CapabilityDenied>`, so "no such artifact" is not merely
//! discouraged as a distinct answer — it is *unrepresentable*.
//!
//! > A read of an artifact the caller is not authorized for MUST return
//! > `CapabilityDenied` whether or not the artifact exists. The daemon MUST NOT
//! > distinguish "no such artifact" from "not yours" — a distinct not-found *is* the
//! > existence oracle.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Existence oracles and
//! >   cross-principal sharing"
//!
//! Three further rules follow, and all three are enforced rather than documented:
//!
//! - **The policy cannot be an oracle.** [`AccessRequest`] carries the caller's token,
//!   what that token confers, and what the caller asked for. It carries no store
//!   reference and no existence bit, so an [`AuthorizationPolicy`] is structurally
//!   incapable of deciding differently for a present and an absent artifact.
//! - **Authorization runs first.** Every entry point authorizes before it consults the
//!   index, so a denied call never depends on store state at all.
//! - **An authorized read of an absent artifact is also `CapabilityDenied`.** Fail-closed
//!   is the only answer that keeps the refusal uniform. A receipt-mediated read of
//!   content that was summarized, purged, or lost reports `Redacted(reason, commitment)`
//!   instead (plan §4.5); that is a different operation over a receipt the caller already
//!   holds, and it belongs to the evidence layer, not to a raw read.
//!
//! Dedup is invisible for the same reason. [`PublicationCost`] is computed at stage time
//! from the request alone, so a publication that converges onto content another principal
//! already holds reports the same cost as a first publication — RFC 0026 requires exactly
//! that, because "a dedup that is invisible in `artifacts` but visible in reported cost is
//! still an oracle". The returned [`PublicationReceipt`] likewise carries no
//! first-or-converged flag; publication order lives only in the ledger, behind
//! [`Action::Audit`].
//!
//! There is deliberately no `contains`, `exists`, or public listing method. Such a method
//! *is* the existence oracle, whatever the docs around it say.
//!
//! # Auditability
//!
//! > Every privileged operation records actor, capability, inputs, policy decision,
//! > outputs, and evidence identity.
//! >
//! > — `notes/plan/plan.md` §18.5
//!
//! Every authorization decision — permit and deny alike — is handed to an [`AuditSink`]
//! before the store acts. [`AuditLog`] is the in-memory implementation. The record is
//! written at *decision* time and names no outcome, which keeps the audit log from
//! becoming the oracle the refusal is not. Tokens are secrets (RFC 0026: they "MUST NOT
//! be logged in request traces, MUST NOT appear in error text"), so [`CapabilityToken`]
//! has no [`Display`](fmt::Display) and its [`Debug`] elides the identity.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! use continuum_workspace::publication::{
//!     ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
//!     ContentIdentifier, IdentityUnavailable, ReferenceStore,
//! };
//! use std::sync::Arc;
//!
//! struct LengthIdentifier;
//! impl ContentIdentifier for LengthIdentifier {
//!     fn identify(
//!         &self,
//!         class: ArtifactClass,
//!         content: &[u8],
//!     ) -> Result<ArtifactHandle, IdentityUnavailable> {
//!         ArtifactHandle::new(class, &format!("len{}", content.len()))
//!             .map_err(|_| IdentityUnavailable)
//!     }
//! }
//!
//! let author = CapabilityToken::mint("k1")?;
//! let store = ReferenceStore::builder(LengthIdentifier, Arc::new(AuditLog::new()))
//!     .capability(CapabilityDescriptor::new(
//!         author.clone(),
//!         ActorId::new("author"),
//!         AuthorityLevel::Propose,
//!     ))
//!     .build();
//!
//! let receipt = store.publish(ArtifactClass::Evidence, b"hello".to_vec(), &author)?;
//! assert_eq!(receipt.handle().to_string(), "ev_len5");
//! assert_eq!(store.read(receipt.handle(), &author)?, b"hello");
//!
//! // A handle is a name. Without a capability it opens nothing.
//! let outsider = CapabilityToken::mint("stolen")?;
//! assert!(store.read(receipt.handle(), &outsider).is_err());
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::artifact_path::{ArtifactClass, ArtifactHandle, ArtifactPath};

// --- principals, authority, capabilities ----------------------------------------------

/// The principal an operation is attributed to (RFC 0026 `ActorId`, plan §18.5).
///
/// An actor is a name for attribution, exactly like an [`ArtifactHandle`] is a name for
/// content: holding one confers nothing. Authority lives in a [`CapabilityDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(String);

impl ActorId {
    /// Name a principal.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }

    /// The principal's name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One of RFC 0027's five authority levels.
///
/// > Five levels — `read < propose < execute < revise-intent < promote` — map onto the
/// > plan §18.2 capability set.
/// >
/// > — `notes/plan/rfcs/0027-agent-tool-protocol.md`, "Authority levels and capabilities"
///
/// The declaration order *is* that chain, and it is this type's [`Ord`], so
/// "at least this level" is a comparison rather than a table lookup. The wire enum is
/// `continuumd`'s (`schemas/continuumd-native-protocol.idl`, `AuthorityLevel`); this is
/// the store-side ordering it maps onto one-for-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorityLevel {
    /// Read published artifacts.
    Read,
    /// Propose artifacts and patches; the level `workspace.create/fork/seal` carries.
    Propose,
    /// Start and steer bounded work.
    Execute,
    /// Revise a protected Intent Contract (plan §5.4); never in a default agent profile.
    ReviseIntent,
    /// Promote evidence (INV-015, B12); never in a default agent profile.
    Promote,
}

impl fmt::Display for AuthorityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Propose => "propose",
            Self::Execute => "execute",
            Self::ReviseIntent => "revise-intent",
            Self::Promote => "promote",
        })
    }
}

/// A `cap_*` capability token: the bearer secret that actually confers authority.
///
/// > Capability tokens (`cap_*`) are minted randomly and do confer authority.
/// >
/// > — `notes/plan/plan.md` §4.4
///
/// This is the *only* thing in this module that authorizes anything. It is deliberately
/// hard to confuse with a name:
///
/// - it can be built only from an [`ArtifactClass::Capability`] handle, so no
///   content-addressed handle can be laundered into one ([`CapabilityToken::from_handle`]);
/// - it has no [`Display`](fmt::Display), and its [`Debug`] elides the identity, because
///   RFC 0026 forbids logging tokens in request traces or error text;
/// - it has no store path at all — [`ArtifactPath::for_handle`] refuses the class.
///
/// Minting takes the identity as an argument rather than drawing one from a random
/// source: entropy is a capability, not ambient (INV-005, ADR-0003). The daemon's minting
/// operation supplies it from an explicit entropy effect (plan §4.5, PR 6+).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityToken(ArtifactHandle);

impl CapabilityToken {
    /// Mint a token from a caller-supplied random identity.
    ///
    /// # Errors
    ///
    /// [`InvalidCapabilityToken::Identity`] when `identity` is not a legal handle
    /// identity (`[A-Za-z0-9_-]+`).
    pub fn mint(identity: &str) -> Result<Self, InvalidCapabilityToken> {
        ArtifactHandle::new(ArtifactClass::Capability, identity)
            .map(Self)
            .map_err(|_| InvalidCapabilityToken::Identity)
    }

    /// Reinterpret an existing handle as a capability token.
    ///
    /// This is the conversion that makes "a handle is never a capability" checkable:
    /// every content-addressed class is rejected, and [`ArtifactClass::Capability`] is
    /// the one class [`ArtifactPath::for_handle`] refuses to place, so a handle obtained
    /// by listing or guessing a store path can never arrive here.
    ///
    /// # Errors
    ///
    /// [`InvalidCapabilityToken::NotACapabilityClass`] for every class but
    /// [`ArtifactClass::Capability`].
    pub fn from_handle(handle: ArtifactHandle) -> Result<Self, InvalidCapabilityToken> {
        if handle.class() == ArtifactClass::Capability {
            Ok(Self(handle))
        } else {
            Err(InvalidCapabilityToken::NotACapabilityClass(handle.class()))
        }
    }
}

/// Elides the secret. RFC 0026: tokens "MUST NOT be logged in request traces, MUST NOT
/// appear in error text".
impl fmt::Debug for CapabilityToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapabilityToken(cap_<redacted>)")
    }
}

/// Why a value is not a usable [`CapabilityToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvalidCapabilityToken {
    /// The identity is not `[A-Za-z0-9_-]+`.
    Identity,
    /// The handle names a content-addressed class, which never confers authority.
    NotACapabilityClass(ArtifactClass),
}

impl fmt::Display for InvalidCapabilityToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => f.write_str("capability identity is not [A-Za-z0-9_-]+"),
            Self::NotACapabilityClass(class) => write!(
                f,
                "artifact class `{class}` is content-addressed and confers no authority (plan §4.4)"
            ),
        }
    }
}

impl core::error::Error for InvalidCapabilityToken {}

/// What a `cap_*` token confers (the IDL's `CapabilityDescriptor`, RFC 0027).
///
/// The IDL descriptor also carries snapshot and intent scope, an expiry, and a delegation
/// depth. Scope beyond artifact class needs the snapshot and intent types (PR 3 and PR 4)
/// and expiry needs a clock, which is an explicit effect and not ambient (INV-005,
/// `continuum-effects-time`); both are therefore absent here rather than approximated.
/// Delegation depth is represented by [`ReferenceStore::mint`]'s refusal to mint above the
/// minting capability's own level.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityDescriptor {
    capability: CapabilityToken,
    actor: ActorId,
    level: AuthorityLevel,
    classes: BTreeSet<ArtifactClass>,
}

impl CapabilityDescriptor {
    /// Describe a capability that is unrestricted by artifact class within its level.
    #[must_use]
    pub fn new(capability: CapabilityToken, actor: ActorId, level: AuthorityLevel) -> Self {
        Self {
            capability,
            actor,
            level,
            classes: BTreeSet::new(),
        }
    }

    /// Restrict this capability to the given artifact classes.
    ///
    /// An empty class set means "unrestricted within `level`", matching the IDL's
    /// `artifact_classes: … empty means all`.
    #[must_use]
    pub fn scoped_to(mut self, classes: impl IntoIterator<Item = ArtifactClass>) -> Self {
        self.classes = classes.into_iter().collect();
        self
    }

    /// The token this descriptor describes.
    #[must_use]
    pub fn capability(&self) -> &CapabilityToken {
        &self.capability
    }

    /// The principal the capability's holder acts as.
    #[must_use]
    pub fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// The authority level conferred.
    #[must_use]
    pub const fn level(&self) -> AuthorityLevel {
        self.level
    }

    /// The artifact classes in scope; empty means all.
    pub fn classes(&self) -> impl Iterator<Item = ArtifactClass> + '_ {
        self.classes.iter().copied()
    }

    /// Whether `class` is within this capability's class scope.
    #[must_use]
    pub fn permits_class(&self, class: ArtifactClass) -> bool {
        self.classes.is_empty() || self.classes.contains(&class)
    }
}

// --- actions, requests, decisions ------------------------------------------------------

/// What a caller is asking the store to do.
///
/// Minimum authority follows RFC 0027's operation registry: `evidence.get/query` is
/// `read`, `workspace.create/fork/seal` — the operations that publish an artifact — is
/// `propose`, and capability administration together with the receipt ledger sit at
/// `promote`, the level RFC 0027 keeps out of default agent profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// Read a published artifact's content.
    Read,
    /// Publish an artifact.
    Publish,
    /// Mint or revoke a capability (plan §4.5, audited).
    Administer,
    /// Inspect the receipt ledger, storage attribution, and the index verifier.
    Audit,
}

impl Action {
    /// The lowest [`AuthorityLevel`] that may perform this action.
    #[must_use]
    pub const fn minimum_authority(self) -> AuthorityLevel {
        match self {
            Self::Read => AuthorityLevel::Read,
            Self::Publish => AuthorityLevel::Propose,
            Self::Administer | Self::Audit => AuthorityLevel::Promote,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Publish => "publish",
            Self::Administer => "administer",
            Self::Audit => "audit",
        })
    }
}

/// Everything an [`AuthorizationPolicy`] is allowed to see.
///
/// The omissions are the design. There is no store reference and no existence flag, so a
/// policy *cannot* answer differently for a present and an absent artifact — the
/// existence-oracle rule of RFC 0026 is a property of this type, not of policy discipline.
/// `handle` is present for reads because scope may name a specific artifact, and absent
/// for publication because RFC 0027 requires authority to be checked "before any semantic
/// work runs", which includes computing the identity.
#[derive(Debug, Clone, Copy)]
pub struct AccessRequest<'a> {
    capability: &'a CapabilityToken,
    descriptor: &'a CapabilityDescriptor,
    action: Action,
    class: Option<ArtifactClass>,
    handle: Option<&'a ArtifactHandle>,
}

impl<'a> AccessRequest<'a> {
    /// The token the caller presented.
    #[must_use]
    pub const fn capability(&self) -> &'a CapabilityToken {
        self.capability
    }

    /// What that token confers.
    #[must_use]
    pub const fn descriptor(&self) -> &'a CapabilityDescriptor {
        self.descriptor
    }

    /// The action requested.
    #[must_use]
    pub const fn action(&self) -> Action {
        self.action
    }

    /// The artifact class in question, when the action names one.
    #[must_use]
    pub const fn class(&self) -> Option<ArtifactClass> {
        self.class
    }

    /// The artifact named, when the action names one. A name, never an authorization.
    #[must_use]
    pub const fn handle(&self) -> Option<&'a ArtifactHandle> {
        self.handle
    }
}

/// A policy's answer. Two-valued on purpose: there is no "not found".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorizationDecision {
    /// The capability covers the request.
    Permitted,
    /// It does not. The caller learns nothing else (see [`CapabilityDenied`]).
    Denied,
}

/// The seam every authorization decision goes through.
///
/// Deployments differ (RFC 0026 makes the cross-principal sharing policy "a capability
/// property rather than a daemon-global flag"), so the rule is injected rather than
/// hard-coded. [`ScopedCapabilityPolicy`] is the default.
pub trait AuthorizationPolicy: Send + Sync {
    /// Decide one request. Called before the store consults its index, and given no way
    /// to consult it.
    fn decide(&self, request: &AccessRequest<'_>) -> AuthorizationDecision;
}

/// The default policy: level ordering plus artifact-class scope.
///
/// Permits when the descriptor's level is at least [`Action::minimum_authority`] and the
/// action's class is within the descriptor's class scope. It additionally refuses to
/// publish an [`ArtifactClass::Capability`]: a bearer token is minted, never published
/// (docs/35, "Storage lifecycle"), and it has no content-derived path to be published to.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScopedCapabilityPolicy;

impl AuthorizationPolicy for ScopedCapabilityPolicy {
    fn decide(&self, request: &AccessRequest<'_>) -> AuthorizationDecision {
        let descriptor = request.descriptor();
        if descriptor.capability() != request.capability() {
            return AuthorizationDecision::Denied;
        }
        if descriptor.level() < request.action().minimum_authority() {
            return AuthorizationDecision::Denied;
        }
        if let Some(class) = request.class() {
            if !descriptor.permits_class(class) {
                return AuthorizationDecision::Denied;
            }
            if request.action() == Action::Publish && !class.is_content_addressed() {
                return AuthorizationDecision::Denied;
            }
        }
        AuthorizationDecision::Permitted
    }
}

// --- audit -----------------------------------------------------------------------------

/// One authorization decision, as plan §18.5 requires it recorded.
///
/// It names the actor (when the token was registered), the capability, the inputs, and
/// the policy decision. It does **not** name the outcome: the record is written at
/// decision time, before the index is consulted, so a reader of the audit log learns no
/// more about what the store holds than the caller did.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthorizationRecord {
    actor: Option<ActorId>,
    capability: CapabilityToken,
    action: Action,
    class: Option<ArtifactClass>,
    handle: Option<ArtifactHandle>,
    decision: AuthorizationDecision,
}

impl AuthorizationRecord {
    /// The principal, when the presented token was a registered one.
    #[must_use]
    pub const fn actor(&self) -> Option<&ActorId> {
        self.actor.as_ref()
    }

    /// The capability presented.
    #[must_use]
    pub const fn capability(&self) -> &CapabilityToken {
        &self.capability
    }

    /// The action requested.
    #[must_use]
    pub const fn action(&self) -> Action {
        self.action
    }

    /// The artifact class in question, when the action named one.
    #[must_use]
    pub const fn class(&self) -> Option<ArtifactClass> {
        self.class
    }

    /// The artifact named, when the action named one.
    #[must_use]
    pub const fn handle(&self) -> Option<&ArtifactHandle> {
        self.handle.as_ref()
    }

    /// The policy decision.
    #[must_use]
    pub const fn decision(&self) -> AuthorizationDecision {
        self.decision
    }
}

/// Where authorization decisions go.
///
/// A seam rather than a concrete log because plan §18.5 requires audit logs to be
/// "append-only and separate from semantic events", and where that append lands is a
/// deployment decision (docs/35 exports them as append-only streams).
pub trait AuditSink: Send + Sync {
    /// Append one decision. Called on both permit and deny, before the store acts.
    fn record(&self, record: AuthorizationRecord);
}

impl<T: AuditSink + ?Sized> AuditSink for std::sync::Arc<T> {
    fn record(&self, record: AuthorizationRecord) {
        (**self).record(record);
    }
}

/// An in-memory append-only [`AuditSink`].
#[derive(Debug, Default)]
pub struct AuditLog {
    records: Mutex<Vec<AuthorizationRecord>>,
}

impl AuditLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every record appended so far, in append order.
    #[must_use]
    pub fn records(&self) -> Vec<AuthorizationRecord> {
        lock(&self.records).clone()
    }

    /// How many decisions have been recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.records).len()
    }

    /// Whether the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        lock(&self.records).is_empty()
    }
}

impl AuditSink for AuditLog {
    fn record(&self, record: AuthorizationRecord) {
        lock(&self.records).push(record);
    }
}

// --- identity seam ---------------------------------------------------------------------

/// The identity function is not this module's decision.
///
/// > Content identity per ADR-0013: in certified lanes, canonical content identity is
/// > primary and hash collisions are resolved by canonical comparison; 256-bit-hash
/// > identity only in explicitly labeled non-certified modes.
/// >
/// > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 2 / IMPL-03
///
/// That is `continuum-value`'s deliverable. This module takes it as an opaque input so
/// that the store never has to trust a publisher's *claimed* identity: the store computes
/// the identity itself, from the content, which is precisely why two concurrent
/// publishers of identical bytes converge — they do not have to agree, and a lying
/// publisher cannot file content under someone else's name.
pub trait ContentIdentifier: Send + Sync {
    /// Derive the handle for `content` in `class`.
    ///
    /// Must be a pure function of its arguments: two calls with equal arguments, in any
    /// process, must return equal handles (INV-005, INV-006).
    ///
    /// # Errors
    ///
    /// [`IdentityUnavailable`] when no identity can be derived; the publication then
    /// aborts atomically rather than proceeding under a guess.
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable>;
}

/// No identity could be derived for the staged content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IdentityUnavailable;

impl fmt::Display for IdentityUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no content identity could be derived")
    }
}

impl core::error::Error for IdentityUnavailable {}

// --- storage faults --------------------------------------------------------------------

/// Where a publication is when something happens to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PublicationPhase {
    /// Authorized, content in hand, identity being derived. Nothing is stored.
    Staging,
    /// Committing content. docs/35 step 5.
    CommittingContent,
    /// Committing the index entry that names it. docs/35 step 6.
    CommittingIndex,
}

impl fmt::Display for PublicationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Staging => "staging",
            Self::CommittingContent => "committing content",
            Self::CommittingIndex => "committing index",
        })
    }
}

/// The seam that makes storage failure testable without a disk.
///
/// > Disk exhaustion during publication aborts atomically (INV-017); it never truncates.
/// >
/// > — `notes/plan/plan.md` §4.5
///
/// docs/35's acceptance list requires "a crash injected between the content commit and
/// the index commit of every publication phase, with fsck run on the survivor". This is
/// where that injection enters; [`NoFaults`] is the default.
pub trait StorageFaults: Send + Sync {
    /// Called at the start of each phase. `Err` aborts the publication atomically.
    ///
    /// # Errors
    ///
    /// Whatever the implementation chooses to inject.
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason>;
}

/// A store whose backing storage never fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFaults;

impl StorageFaults for NoFaults {
    fn check(&self, _phase: PublicationPhase) -> Result<(), AbortReason> {
        Ok(())
    }
}

// --- refusals --------------------------------------------------------------------------

/// RFC 0026's `CapabilityDenied`, and the whole of what a refused caller learns.
///
/// A unit struct has exactly one value, so this type carries zero bits. That is the
/// point: "a read of an artifact the caller is not authorized for MUST return
/// `CapabilityDenied` whether or not the artifact exists" (RFC 0026) is not a rule the
/// store follows, it is a rule the store's return type cannot break. `Result<T,
/// CapabilityDenied>` has no room for a not-found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CapabilityDenied;

impl fmt::Display for CapabilityDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No handle, no class, no actor: error text is part of the oracle surface, and
        // RFC 0026 also forbids capability tokens from appearing in it.
        f.write_str("capability denied")
    }
}

impl core::error::Error for CapabilityDenied {}

/// Why an atomic publication aborted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbortReason {
    /// Backing storage could not take the write (plan §4.5's disk-exhaustion case).
    StorageExhausted,
    /// No content identity could be derived ([`IdentityUnavailable`]).
    IdentityUnavailable,
    /// The identifier returned a handle of a different class than the one staged.
    IdentityClassMismatch,
    /// The class is not content-addressed, so it has no place in the store (plan §4.4).
    NotContentAddressed,
    /// The identity already names byte-different content.
    ///
    /// ADR-0013's exact comparison, store side: two artifacts that are not equal are not
    /// the same artifact, whatever their hashes say. Conflating them would be the failure
    /// docs/35's index verifier calls "identity mismatch" and forbids repairing silently.
    IdentityCollision,
    /// The publication was dropped or abandoned before it completed — the crash case.
    Abandoned,
}

impl fmt::Display for AbortReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StorageExhausted => "backing storage exhausted",
            Self::IdentityUnavailable => "no content identity could be derived",
            Self::IdentityClassMismatch => "derived identity has a different artifact class",
            Self::NotContentAddressed => "artifact class is not content-addressed",
            Self::IdentityCollision => "identity already names byte-different content",
            Self::Abandoned => "publication abandoned before it completed",
        })
    }
}

/// RFC 0026's `PublicationAborted`, at the grain of one artifact: nothing was published
/// and nothing was truncated.
///
/// > A publication that cannot complete atomically MUST fail with `PublicationAborted`.
/// > The artifact whose publication aborted is not published and not truncated: it has no
/// > index entry and no receipt, and no reader can fetch it.
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Atomicity of publication"
///
/// This type is one artifact's abort, so "nothing is published" is exact here: no index
/// entry, no receipt, nothing a reader can observe. A composite of several publications
/// ([`crate::seal`]) is ordered, not transactional, and RFC 0026 states what an abort at
/// record *k* may leave published. An abort after the content commit may leave *unreachable* content, which is
/// GC residue rather than a partial artifact — docs/35 chooses that asymmetry
/// deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicationAborted {
    phase: PublicationPhase,
    reason: AbortReason,
}

impl PublicationAborted {
    const fn new(phase: PublicationPhase, reason: AbortReason) -> Self {
        Self { phase, reason }
    }

    /// The phase the publication reached.
    #[must_use]
    pub const fn phase(&self) -> PublicationPhase {
        self.phase
    }

    /// Why it aborted.
    #[must_use]
    pub const fn reason(&self) -> AbortReason {
        self.reason
    }
}

impl fmt::Display for PublicationAborted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "publication aborted while {}: {} (INV-017: nothing published, nothing truncated)",
            self.phase, self.reason
        )
    }
}

impl core::error::Error for PublicationAborted {}

/// Why a publication did not produce a receipt.
///
/// Exactly two ways, matching RFC 0026's error taxonomy: the caller was not authorized,
/// or the publication aborted atomically. There is no third answer, and in particular no
/// "already exists" — a publication of content the store already holds succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PublishRefusal {
    /// The capability did not cover the publication (RFC 0026 `CapabilityDenied`).
    CapabilityDenied(CapabilityDenied),
    /// The publication aborted (RFC 0026 `PublicationAborted`).
    Aborted(PublicationAborted),
}

impl From<CapabilityDenied> for PublishRefusal {
    fn from(denied: CapabilityDenied) -> Self {
        Self::CapabilityDenied(denied)
    }
}

impl From<PublicationAborted> for PublishRefusal {
    fn from(aborted: PublicationAborted) -> Self {
        Self::Aborted(aborted)
    }
}

impl fmt::Display for PublishRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityDenied(denied) => denied.fmt(f),
            Self::Aborted(aborted) => aborted.fmt(f),
        }
    }
}

impl core::error::Error for PublishRefusal {}

// --- receipts --------------------------------------------------------------------------

/// What a publication cost, as reported to its publisher.
///
/// A pure function of the staged request — class and content length — and of nothing the
/// store already holds. RFC 0026: "publishing content already held by another principal
/// produces the same envelope and `cost` as a first publication", because "a dedup that
/// is invisible in `artifacts` but visible in reported cost is still an oracle".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicationCost {
    content_bytes: u64,
    index_entries: u64,
}

impl PublicationCost {
    /// The cost of publishing `content`, computed from the request alone.
    #[must_use]
    pub fn for_content(content: &[u8]) -> Self {
        Self {
            content_bytes: u64::try_from(content.len()).unwrap_or(u64::MAX),
            index_entries: 1,
        }
    }

    /// Bytes of content the publication accounted for.
    #[must_use]
    pub const fn content_bytes(&self) -> u64 {
        self.content_bytes
    }

    /// Index entries the publication accounted for. Always one.
    #[must_use]
    pub const fn index_entries(&self) -> u64 {
        self.index_entries
    }
}

/// Proof that a named artifact was published, issued to the publisher.
///
/// Deliberately silent about publication order. It carries no sequence number and no
/// first-or-converged flag, so a publisher cannot tell from its own receipt whether it
/// created the identity or converged onto someone else's — which is the existence-oracle
/// rule of plan §4.5 applied to the success path. Order exists, in the ledger, behind
/// [`Action::Audit`].
///
/// It also carries no storage attribution: RFC 0026 makes attribution "operational
/// telemetry" that "MUST NOT appear inside a receipt". See
/// [`StoreAudit::storage_attribution`].
///
/// # Only the index commit mints one (INV-017)
///
/// A receipt is the provenance half of a publication, so a receipt that no index commit
/// issued would be provenance for nothing. [`CommittedContent::commit_index`] is its one
/// constructor. Every value a receipt holds is public to build:
///
/// ```
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::*;
/// let handle = ArtifactHandle::new(ArtifactClass::Evidence, "forged").unwrap();
/// let actor = ActorId::new("forger");
/// let capability = CapabilityToken::mint("k1").unwrap();
/// let cost = PublicationCost::for_content(b"");
/// # let _ = (handle, actor, capability, cost);
/// ```
///
/// but the fields are private, so the receipt itself cannot be built from them outside
/// this crate:
///
/// ```compile_fail,E0451
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::*;
/// let forged = PublicationReceipt {
///     handle: ArtifactHandle::new(ArtifactClass::Evidence, "forged").unwrap(),
///     actor: ActorId::new("forger"),
///     capability: CapabilityToken::mint("k1").unwrap(),
///     cost: PublicationCost::for_content(b""),
/// };
/// ```
///
/// Inside the crate, `tests/inv017_publication_atomicity_evidence.rs` keeps the one
/// constructor the only one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicationReceipt {
    handle: ArtifactHandle,
    actor: ActorId,
    capability: CapabilityToken,
    cost: PublicationCost,
}

impl PublicationReceipt {
    /// The published artifact's handle — the one semantic identity.
    #[must_use]
    pub const fn handle(&self) -> &ArtifactHandle {
        &self.handle
    }

    /// The principal the publication is attributed to.
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// The capability the publication was performed under (plan §18.5).
    #[must_use]
    pub const fn capability(&self) -> &CapabilityToken {
        &self.capability
    }

    /// What the publication cost.
    #[must_use]
    pub const fn cost(&self) -> PublicationCost {
        self.cost
    }
}

// --- names tied to a receipt ---------------------------------------------------------------

/// A name that can spell a published artifact's handle.
///
/// The consumer's half of [`Published`]. A daemon names artifacts with its own handle
/// types — one per wire class — and the store names them with one [`ArtifactHandle`]. This
/// trait is the one comparison between the two spellings, so [`Published::attest`] can
/// refuse a name that is not the receipt's.
///
/// An implementation MUST answer `true` exactly when `self` and `handle` spell one
/// identity. It is a pure function of its two arguments.
pub trait PublishedName {
    /// Whether this name spells `handle`.
    fn names(&self, handle: &ArtifactHandle) -> bool;
}

impl PublishedName for ArtifactHandle {
    fn names(&self, handle: &ArtifactHandle) -> bool {
        self == handle
    }
}

/// A name that is not the handle of the receipt it was offered with.
///
/// [`fmt::Display`] names neither side: the name can come off the wire (INV-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NameNotPublished;

impl fmt::Display for NameNotPublished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the name is not the handle the publication receipt was issued for")
    }
}

impl core::error::Error for NameNotPublished {}

/// A handle that names a **published** artifact, tied at the type level to its receipt
/// (INV-017, bn-283p6).
///
/// A consumer that records "this artifact is published" in its own volatile state — a
/// daemon's workspace record, task record, or evidence node — holds this type in place of
/// the bare handle. A value exists only after a [`PublicationReceipt`] for the same
/// identity existed, and a receipt exists only after [`CommittedContent::commit_index`]
/// wrote the index and the ledger. So a record that holds a `Published<H>` cannot be
/// written before the publication it names has committed. The order is a property of
/// what compiles, not of the order of two statements.
///
/// It is a *name*, not the receipt. It carries no actor, capability, or cost, and it
/// confers no authority (plan §4.4). At a wire boundary it projects to its handle through
/// [`handle`](Self::handle) or [`into_handle`](Self::into_handle). The wire shape does
/// not change.
///
/// # Only a receipt mints one
///
/// [`attest`](Self::attest) takes a receipt and a name, and refuses a name that is not the
/// receipt's handle. [`of`](Self::of) takes the receipt's own handle. There is no other
/// constructor, and the field is private, so a caller outside this crate cannot build one
/// from a bare name:
///
/// ```compile_fail,E0451
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::Published;
/// let forged = Published {
///     name: ArtifactHandle::new(ArtifactClass::Evidence, "never-published").unwrap(),
/// };
/// ```
///
/// Inside the crate, `tests/inv017_publication_atomicity_evidence.rs` keeps these two
/// constructors the only ones, and keeps the consumer records that name a published
/// artifact holding this type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Published<H> {
    name: H,
}

impl<H: PublishedName> Published<H> {
    /// The name `name`, as published by the publication `receipt` records.
    ///
    /// # Errors
    ///
    /// [`NameNotPublished`] when `name` does not spell `receipt`'s handle. A receipt for
    /// one artifact never attests a name of another.
    pub fn attest(receipt: &PublicationReceipt, name: H) -> Result<Self, NameNotPublished> {
        if name.names(receipt.handle()) {
            Ok(Self { name })
        } else {
            Err(NameNotPublished)
        }
    }
}

impl Published<ArtifactHandle> {
    /// The store's own name for the artifact `receipt` records.
    #[must_use]
    pub fn of(receipt: &PublicationReceipt) -> Self {
        Self {
            name: receipt.handle().clone(),
        }
    }
}

impl<H> Published<H> {
    /// The handle: the projection a wire value or a lookup key reads.
    #[must_use]
    pub const fn handle(&self) -> &H {
        &self.name
    }

    /// The handle, taken by value.
    #[must_use]
    pub fn into_handle(self) -> H {
        self.name
    }
}

// --- the store -------------------------------------------------------------------------

#[derive(Debug, Default)]
struct StoreState {
    capabilities: BTreeMap<CapabilityToken, CapabilityDescriptor>,
    /// Committed content, keyed by identity. Present here and absent from `index` is
    /// exactly docs/35's "unreachable content".
    content: BTreeMap<ArtifactHandle, Vec<u8>>,
    /// Publications that have committed their content and not yet their index entry,
    /// counted per identity — docs/35's "live tasks" half of the reachability root set.
    ///
    /// A count rather than a set because convergent publishers share an identity: N
    /// threads publishing identical bytes hold N pins on one handle, and the content stops
    /// being a root only when the last of them settles. One pin is taken by
    /// [`StagedPublication::commit_content`], in the same critical section that makes the
    /// content durable, and released when the resulting [`CommittedContent`] is consumed
    /// or dropped.
    in_flight: BTreeMap<ArtifactHandle, u64>,
    /// The index: the only thing a read consults. Never written before content.
    index: BTreeMap<ArtifactPath, ArtifactHandle>,
    /// Append-only receipt ledger, keyed by identity, in publication order.
    ledger: BTreeMap<ArtifactHandle, Vec<PublicationReceipt>>,
    /// Every abort, including abandoned publications.
    aborts: Vec<PublicationAborted>,
}

/// An in-memory store implementing the publication state machine and the authorization
/// rules of plan §4.4–§4.5.
///
/// It is the semantic reference, not the daemon: `continuumd` (PR 6+) must reproduce
/// these observable behaviors over durable storage. Every method takes `&self` and the
/// store is [`Sync`], so the concurrency obligations are exercised the way they will be
/// met — by many threads against one store.
///
/// Build one with [`ReferenceStore::builder`].
pub struct ReferenceStore {
    identifier: Box<dyn ContentIdentifier>,
    policy: Box<dyn AuthorizationPolicy>,
    audit: Box<dyn AuditSink>,
    faults: Box<dyn StorageFaults>,
    state: Mutex<StoreState>,
}

impl fmt::Debug for ReferenceStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately opaque: a `Debug` that printed the index would be an existence
        // oracle reachable without any capability at all.
        f.write_str("ReferenceStore { .. }")
    }
}

impl ReferenceStore {
    /// Start building a store.
    ///
    /// The identifier and the audit sink are required, not defaulted: identity is
    /// `continuum-value`'s decision and must be supplied by the deployment (ADR-0013),
    /// and plan §18.5 admits no unaudited privileged operation, so there is no
    /// "no audit" default to fall into.
    pub fn builder(
        identifier: impl ContentIdentifier + 'static,
        audit: impl AuditSink + 'static,
    ) -> ReferenceStoreBuilder {
        ReferenceStoreBuilder {
            identifier: Box::new(identifier),
            audit: Box::new(audit),
            policy: Box::new(ScopedCapabilityPolicy),
            faults: Box::new(NoFaults),
            capabilities: Vec::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, StoreState> {
        lock(&self.state)
    }

    /// Authenticate the token, ask the policy, record the decision, answer.
    ///
    /// The order is load-bearing. The registry lookup is keyed by the caller's own token
    /// and reveals nothing about any artifact; the policy sees no store state at all; and
    /// the index is not touched until after this returns. A denial is therefore a
    /// function of the request alone.
    fn authorize(
        &self,
        capability: &CapabilityToken,
        action: Action,
        class: Option<ArtifactClass>,
        handle: Option<&ArtifactHandle>,
    ) -> Result<ActorId, CapabilityDenied> {
        let descriptor = self.state().capabilities.get(capability).cloned();

        let Some(descriptor) = descriptor else {
            // An unregistered or revoked token is authenticated away before any policy
            // runs, and the denial is still audited (plan §18.5).
            self.audit.record(AuthorizationRecord {
                actor: None,
                capability: capability.clone(),
                action,
                class,
                handle: handle.cloned(),
                decision: AuthorizationDecision::Denied,
            });
            return Err(CapabilityDenied);
        };

        let decision = self.policy.decide(&AccessRequest {
            capability,
            descriptor: &descriptor,
            action,
            class,
            handle,
        });

        self.audit.record(AuthorizationRecord {
            actor: Some(descriptor.actor().clone()),
            capability: capability.clone(),
            action,
            class,
            handle: handle.cloned(),
            decision,
        });

        match decision {
            AuthorizationDecision::Permitted => Ok(descriptor.actor().clone()),
            AuthorizationDecision::Denied => Err(CapabilityDenied),
        }
    }

    fn record_abort(&self, aborted: PublicationAborted) {
        self.state().aborts.push(aborted);
    }

    /// Release one publication's claim on `handle` as a garbage-collection root.
    ///
    /// The counterpart of the pin [`StagedPublication::commit_content`] takes. Called from
    /// [`CommittedContent`]'s [`Drop`], which every exit from that state runs through —
    /// success, abandonment, and refused index write alike — so a pin cannot outlive the
    /// publication holding it.
    fn release_in_flight(&self, handle: &ArtifactHandle) {
        let mut state = self.state();
        if let Some(pins) = state.in_flight.get_mut(handle) {
            *pins = pins.saturating_sub(1);
            if *pins == 0 {
                state.in_flight.remove(handle);
            }
        }
    }

    fn abort(&self, phase: PublicationPhase, reason: AbortReason) -> PublicationAborted {
        let aborted = PublicationAborted::new(phase, reason);
        self.record_abort(aborted);
        aborted
    }

    /// Stage a publication: authorize, then derive the identity.
    ///
    /// Nothing is stored yet. The returned [`StagedPublication`] is the first state of
    /// the machine; dropping it aborts.
    ///
    /// # Errors
    ///
    /// [`PublishRefusal::CapabilityDenied`] when the capability does not cover publishing
    /// this class, and [`PublishRefusal::Aborted`] when the class is not content-addressed
    /// or no identity could be derived.
    pub fn stage(
        &self,
        class: ArtifactClass,
        content: Vec<u8>,
        capability: &CapabilityToken,
    ) -> Result<StagedPublication<'_>, PublishRefusal> {
        let actor = self.authorize(capability, Action::Publish, Some(class), None)?;

        if !class.is_content_addressed() {
            return Err(self
                .abort(PublicationPhase::Staging, AbortReason::NotContentAddressed)
                .into());
        }

        self.faults
            .check(PublicationPhase::Staging)
            .map_err(|reason| self.abort(PublicationPhase::Staging, reason))?;

        let handle = self
            .identifier
            .identify(class, &content)
            .map_err(|IdentityUnavailable| {
                self.abort(PublicationPhase::Staging, AbortReason::IdentityUnavailable)
            })?;

        if handle.class() != class {
            return Err(self
                .abort(
                    PublicationPhase::Staging,
                    AbortReason::IdentityClassMismatch,
                )
                .into());
        }

        let cost = PublicationCost::for_content(&content);
        Ok(StagedPublication {
            store: self,
            handle,
            content: Some(content),
            actor,
            capability: capability.clone(),
            cost,
            settled: false,
        })
    }

    /// Stage, commit content, commit index.
    ///
    /// The convenience spelling of the state machine, for callers with no reason to hold
    /// an intermediate state. Identical in effect to the three steps run in order.
    ///
    /// # Errors
    ///
    /// The errors of [`ReferenceStore::stage`], [`StagedPublication::commit_content`],
    /// and [`CommittedContent::commit_index`].
    pub fn publish(
        &self,
        class: ArtifactClass,
        content: Vec<u8>,
        capability: &CapabilityToken,
    ) -> Result<PublicationReceipt, PublishRefusal> {
        Ok(self
            .stage(class, content, capability)?
            .commit_content()?
            .commit_index()?)
    }

    /// Read a published artifact.
    ///
    /// Requires a capability. The handle is a name, and a name is not authorization
    /// (plan §4.4, ADR-0037).
    ///
    /// # Errors
    ///
    /// [`CapabilityDenied`] — the only thing this can return — when the capability does
    /// not cover the read, **and equally** when the artifact is not published. There is no
    /// not-found: RFC 0026 forbids distinguishing "no such artifact" from "not yours",
    /// and the return type makes the distinction unrepresentable.
    pub fn read(
        &self,
        handle: &ArtifactHandle,
        capability: &CapabilityToken,
    ) -> Result<Vec<u8>, CapabilityDenied> {
        self.authorize(capability, Action::Read, Some(handle.class()), Some(handle))?;

        // Only now is the store consulted. A `cap_*` handle has no path; that too is a
        // denial, never a distinguishable error.
        let path = ArtifactPath::for_handle(handle).map_err(|_| CapabilityDenied)?;
        let state = self.state();
        match state.index.get(&path) {
            Some(indexed) if indexed == handle => {
                state.content.get(handle).cloned().ok_or(CapabilityDenied)
            }
            _ => Err(CapabilityDenied),
        }
    }

    /// Mint a capability (plan §4.5, audited).
    ///
    /// `administrator` must itself confer [`Action::Administer`], and it may not mint
    /// above its own level — the store-side form of RFC 0027's delegation rule that a
    /// capability names "delegation allowance", and of INV-015's least authority.
    ///
    /// The new token's identity is chosen by the caller: entropy is an explicit capability
    /// (INV-005, ADR-0003), never something a store reaches for.
    ///
    /// # Errors
    ///
    /// [`CapabilityDenied`] when the administrator does not confer administration or would
    /// escalate.
    pub fn mint(
        &self,
        administrator: &CapabilityToken,
        descriptor: CapabilityDescriptor,
    ) -> Result<CapabilityToken, CapabilityDenied> {
        self.authorize(administrator, Action::Administer, None, None)?;

        let mut state = self.state();
        let admin_level = state
            .capabilities
            .get(administrator)
            .ok_or(CapabilityDenied)?
            .level();
        if descriptor.level() > admin_level {
            return Err(CapabilityDenied);
        }

        let token = descriptor.capability().clone();
        state.capabilities.insert(token.clone(), descriptor);
        Ok(token)
    }

    /// Revoke a capability (plan §4.5; docs/35: "Revocation, not purge, is how a
    /// capability stops being usable").
    ///
    /// # Errors
    ///
    /// [`CapabilityDenied`] when the administrator does not confer administration.
    /// Revoking a token that was never registered succeeds silently, so that probing the
    /// registry through this operation reveals nothing.
    pub fn revoke(
        &self,
        administrator: &CapabilityToken,
        capability: &CapabilityToken,
    ) -> Result<(), CapabilityDenied> {
        self.authorize(administrator, Action::Administer, None, None)?;
        self.state().capabilities.remove(capability);
        Ok(())
    }

    /// Reclaim unreachable content (plan §4.5: "Artifacts are garbage-collected by
    /// reachability").
    ///
    /// Unreachable content is the residue docs/35 accepts in exchange for never writing a
    /// stale index entry. Collecting it cannot make a published artifact unavailable, and
    /// cannot make one that is *about to be* published unavailable either, because the
    /// root set is both halves of docs/35's — "named roots, live tasks, receipts, and
    /// retention policy" — spelled for this store:
    ///
    /// - **named roots** are the index entries: an artifact is published exactly when the
    ///   index names it, so every published identity is a root;
    /// - **live tasks** are the publications between their two commits. Content is durable
    ///   from [`StagedPublication::commit_content`], and until the matching
    ///   [`CommittedContent::commit_index`] no index entry names it — so for that window
    ///   the publication is its own root, held by the pin `commit_content` takes and
    ///   released when [`CommittedContent`] is consumed or dropped. Without it, collection
    ///   racing a publication would reclaim content the store had already told a publisher
    ///   was durable: the G0-DX-13 defect reproduced in
    ///   `crates/continuum-workspace/tests/dx13_falsification.rs`;
    /// - **receipts** need no separate root and are deliberately not one. A receipt is
    ///   appended in the same critical section that inserts the index entry naming it, an
    ///   [`ArtifactPath`] determines its handle so that entry can name no other identity,
    ///   and nothing in this store ever removes an index entry. Every receipted identity
    ///   is therefore already a named root; adding receipts would be dead machinery;
    /// - **retention policy** is a deployment's, not this store's: there is no
    ///   time here to express one with, and time is an explicit effect (INV-005,
    ///   ADR-0003).
    ///
    /// What is left over — content that no index entry names and no live publication is
    /// holding — is crash residue, and only that is reclaimed.
    ///
    /// Returns the identities reclaimed, in store order.
    ///
    /// # Errors
    ///
    /// [`CapabilityDenied`] when the capability does not confer administration.
    pub fn collect_garbage(
        &self,
        administrator: &CapabilityToken,
    ) -> Result<Vec<ArtifactHandle>, CapabilityDenied> {
        self.authorize(administrator, Action::Administer, None, None)?;

        let mut state = self.state();
        let reachable: BTreeSet<ArtifactHandle> = state
            .index
            .values()
            .cloned()
            .chain(state.in_flight.keys().cloned())
            .collect();
        let unreachable: Vec<ArtifactHandle> = state
            .content
            .keys()
            .filter(|handle| !reachable.contains(*handle))
            .cloned()
            .collect();
        for handle in &unreachable {
            state.content.remove(handle);
        }
        Ok(unreachable)
    }

    /// Open the operator view: receipt ledger, storage attribution, index verifier.
    ///
    /// Behind [`Action::Audit`] (minimum authority `promote`) because everything it
    /// exposes is an existence oracle by construction. Ordinary clients get receipts for
    /// their own publications and nothing else.
    ///
    /// # Errors
    ///
    /// [`CapabilityDenied`] when the capability does not confer audit.
    pub fn audit_view(
        &self,
        capability: &CapabilityToken,
    ) -> Result<StoreAudit<'_>, CapabilityDenied> {
        self.authorize(capability, Action::Audit, None, None)?;
        Ok(StoreAudit { store: self })
    }
}

/// Builder for [`ReferenceStore`].
pub struct ReferenceStoreBuilder {
    identifier: Box<dyn ContentIdentifier>,
    audit: Box<dyn AuditSink>,
    policy: Box<dyn AuthorizationPolicy>,
    faults: Box<dyn StorageFaults>,
    capabilities: Vec<CapabilityDescriptor>,
}

impl fmt::Debug for ReferenceStoreBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReferenceStoreBuilder { .. }")
    }
}

impl ReferenceStoreBuilder {
    /// Replace the default [`ScopedCapabilityPolicy`].
    #[must_use]
    pub fn policy(mut self, policy: impl AuthorizationPolicy + 'static) -> Self {
        self.policy = Box::new(policy);
        self
    }

    /// Replace the default [`NoFaults`] storage.
    #[must_use]
    pub fn faults(mut self, faults: impl StorageFaults + 'static) -> Self {
        self.faults = Box::new(faults);
        self
    }

    /// Register a capability the deployment declares up front.
    ///
    /// A store with no registered capability can do nothing at all, which is the correct
    /// fail-closed starting point: authority is granted, never ambient.
    #[must_use]
    pub fn capability(mut self, descriptor: CapabilityDescriptor) -> Self {
        self.capabilities.push(descriptor);
        self
    }

    /// Build the store.
    #[must_use]
    pub fn build(self) -> ReferenceStore {
        let capabilities = self
            .capabilities
            .into_iter()
            .map(|descriptor| (descriptor.capability().clone(), descriptor))
            .collect();
        ReferenceStore {
            identifier: self.identifier,
            policy: self.policy,
            audit: self.audit,
            faults: self.faults,
            state: Mutex::new(StoreState {
                capabilities,
                ..StoreState::default()
            }),
        }
    }
}

// --- the state machine -----------------------------------------------------------------

/// A publication that has been authorized and named, but not stored.
///
/// The first state of the machine. [`StagedPublication::commit_content`] consumes it and
/// is the only way to reach [`CommittedContent`]; dropping it aborts with
/// [`AbortReason::Abandoned`] and leaves the store untouched.
#[must_use = "a staged publication that is dropped is an abandoned publication"]
pub struct StagedPublication<'store> {
    store: &'store ReferenceStore,
    handle: ArtifactHandle,
    content: Option<Vec<u8>>,
    actor: ActorId,
    capability: CapabilityToken,
    cost: PublicationCost,
    settled: bool,
}

impl fmt::Debug for StagedPublication<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagedPublication")
            .field("handle", &self.handle)
            .field("actor", &self.actor)
            .field("cost", &self.cost)
            .finish_non_exhaustive()
    }
}

impl<'store> StagedPublication<'store> {
    /// The identity this content will be published under.
    ///
    /// Known before anything is stored, and identical for every publisher of identical
    /// content: the store derived it, the publisher did not assert it.
    #[must_use]
    pub const fn handle(&self) -> &ArtifactHandle {
        &self.handle
    }

    /// What this publication will cost, already fixed by the request alone.
    #[must_use]
    pub const fn cost(&self) -> PublicationCost {
        self.cost
    }

    /// Commit the content — docs/35 step 5, and the *first* durable write.
    ///
    /// Converges when the identity already names byte-identical content: the content is
    /// already durable, so there is nothing to write and nothing to overwrite.
    ///
    /// Committing also pins the identity as a garbage-collection root for as long as the
    /// returned [`CommittedContent`] is alive, in the same critical section that makes the
    /// content durable — there is no instant at which the content exists and nothing
    /// claims it. A converging publication takes a pin too: it is about to be handed a
    /// receipt for content it did not write, and a receipt must name a readable artifact
    /// however the bytes got there.
    ///
    /// # Errors
    ///
    /// [`PublicationAborted`] with [`AbortReason::StorageExhausted`] when the storage
    /// seam refuses the write, and with [`AbortReason::IdentityCollision`] when the
    /// identity already names content that is not byte-identical (ADR-0013 — the store
    /// compares exactly rather than trusting the identity function).
    pub fn commit_content(mut self) -> Result<CommittedContent<'store>, PublicationAborted> {
        self.settled = true;
        let content = self.content.take().unwrap_or_default();
        let store = self.store;

        store
            .faults
            .check(PublicationPhase::CommittingContent)
            .map_err(|reason| store.abort(PublicationPhase::CommittingContent, reason))?;

        {
            let mut state = store.state();
            match state.content.get(&self.handle) {
                Some(existing) if existing.as_slice() != content.as_slice() => {
                    drop(state);
                    return Err(store.abort(
                        PublicationPhase::CommittingContent,
                        AbortReason::IdentityCollision,
                    ));
                }
                // Byte-identical: already durable. Converge without rewriting.
                Some(_) => {}
                None => {
                    state.content.insert(self.handle.clone(), content);
                }
            }
            // Durable, and not yet named by the index: from here until the returned
            // `CommittedContent` settles, this publication is the content's only root.
            *state.in_flight.entry(self.handle.clone()).or_insert(0) += 1;
        }

        Ok(CommittedContent {
            store,
            handle: self.handle.clone(),
            actor: self.actor.clone(),
            capability: self.capability.clone(),
            cost: self.cost,
            settled: false,
        })
    }

    /// Abandon before storing anything.
    pub fn abandon(mut self) -> PublicationAborted {
        self.settled = true;
        self.store
            .abort(PublicationPhase::Staging, AbortReason::Abandoned)
    }
}

impl Drop for StagedPublication<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.store
                .abort(PublicationPhase::Staging, AbortReason::Abandoned);
        }
    }
}

/// The witness that content is durably committed — and the only key to the index.
///
/// This type exists so that "content before index" is a fact about the program rather
/// than a rule about the programmer. [`StagedPublication::commit_content`] is the sole
/// constructor, [`CommittedContent::commit_index`] is the sole index write, and no value
/// of this type can be conjured without having performed the content commit first.
///
/// Dropping it is the crash docs/35 describes: content is durable, no index entry names
/// it, no receipt was issued. [`StoreAudit::fsck`] classifies the residue as
/// [`StoreDefect::UnreachableContent`] and [`ReferenceStore::collect_garbage`] reclaims it.
///
/// # It is also the publication's garbage-collection root
///
/// While this value is alive its content is durable and *nothing else in the store makes
/// it reachable* — the index entry that will name it has not been written yet. So it is a
/// root in its own right, which is docs/35's "live tasks" clause of the root set: the pin
/// is taken by [`StagedPublication::commit_content`] and released by this value's
/// [`Drop`], and [`ReferenceStore::collect_garbage`] running concurrently on another
/// thread therefore cannot reclaim content this publication has already been promised.
///
/// The lifetime of the pin is the lifetime of this value, which is why it is a value at
/// all rather than a flag: there is no exit from this state — receipt, abandonment,
/// refused index write, unwind — that can forget to release it.
///
/// # The ordering is checked, not asserted
///
/// The three steps in order:
///
/// ```
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::*;
/// # use std::sync::Arc;
/// # struct Id;
/// # impl ContentIdentifier for Id {
/// #     fn identify(&self, class: ArtifactClass, content: &[u8])
/// #         -> Result<ArtifactHandle, IdentityUnavailable> {
/// #         ArtifactHandle::new(class, &format!("len{}", content.len()))
/// #             .map_err(|_| IdentityUnavailable)
/// #     }
/// # }
/// # let author = CapabilityToken::mint("k1").unwrap();
/// # let store = ReferenceStore::builder(Id, Arc::new(AuditLog::new()))
/// #     .capability(CapabilityDescriptor::new(
/// #         author.clone(), ActorId::new("a"), AuthorityLevel::Propose))
/// #     .build();
/// let staged = store.stage(ArtifactClass::Evidence, b"x".to_vec(), &author).unwrap();
/// let receipt = staged.commit_content().unwrap().commit_index().unwrap();
/// assert_eq!(store.read(receipt.handle(), &author).unwrap(), b"x");
/// ```
///
/// Skipping the content commit is not a bug that review has to catch. It is a program
/// that does not compile, because `commit_index` is a method on this type and a staged
/// publication is not one:
///
/// ```compile_fail
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::*;
/// # use std::sync::Arc;
/// # struct Id;
/// # impl ContentIdentifier for Id {
/// #     fn identify(&self, class: ArtifactClass, content: &[u8])
/// #         -> Result<ArtifactHandle, IdentityUnavailable> {
/// #         ArtifactHandle::new(class, &format!("len{}", content.len()))
/// #             .map_err(|_| IdentityUnavailable)
/// #     }
/// # }
/// # let author = CapabilityToken::mint("k1").unwrap();
/// # let store = ReferenceStore::builder(Id, Arc::new(AuditLog::new()))
/// #     .capability(CapabilityDescriptor::new(
/// #         author.clone(), ActorId::new("a"), AuthorityLevel::Propose))
/// #     .build();
/// let staged = store.stage(ArtifactClass::Evidence, b"x".to_vec(), &author).unwrap();
/// let receipt = staged.commit_index().unwrap();
/// ```
///
/// Nor can the witness be forged to skip the content commit. Its fields are private, so a
/// caller outside this crate cannot build one:
///
/// ```compile_fail,E0451
/// # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
/// # use continuum_workspace::publication::*;
/// # use std::sync::Arc;
/// # struct Id;
/// # impl ContentIdentifier for Id {
/// #     fn identify(&self, class: ArtifactClass, content: &[u8])
/// #         -> Result<ArtifactHandle, IdentityUnavailable> {
/// #         ArtifactHandle::new(class, &format!("len{}", content.len()))
/// #             .map_err(|_| IdentityUnavailable)
/// #     }
/// # }
/// # let author = CapabilityToken::mint("k1").unwrap();
/// # let store = ReferenceStore::builder(Id, Arc::new(AuditLog::new()))
/// #     .capability(CapabilityDescriptor::new(
/// #         author.clone(), ActorId::new("a"), AuthorityLevel::Propose))
/// #     .build();
/// let forged = CommittedContent {
///     store: &store,
///     handle: ArtifactHandle::new(ArtifactClass::Evidence, "len1").unwrap(),
///     actor: ActorId::new("a"),
///     capability: author.clone(),
///     cost: PublicationCost::for_content(b"x"),
///     settled: false,
/// };
/// let receipt = forged.commit_index().unwrap();
/// ```
#[must_use = "dropping committed content leaves unreachable content and no receipt"]
pub struct CommittedContent<'store> {
    store: &'store ReferenceStore,
    handle: ArtifactHandle,
    actor: ActorId,
    capability: CapabilityToken,
    cost: PublicationCost,
    settled: bool,
}

impl fmt::Debug for CommittedContent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommittedContent")
            .field("handle", &self.handle)
            .field("actor", &self.actor)
            .field("cost", &self.cost)
            .finish_non_exhaustive()
    }
}

impl CommittedContent<'_> {
    /// The identity whose content is now durable.
    #[must_use]
    pub const fn handle(&self) -> &ArtifactHandle {
        &self.handle
    }

    /// Commit the index entry — docs/35 step 6 — and issue the receipt.
    ///
    /// This is the instant the artifact becomes visible (INV-017). The index entry is
    /// created by the first publisher to arrive and left alone by every later one; since
    /// the entry is derived from the identity, "first wins" and "all converge" are the
    /// same statement. Every publisher's receipt is appended to the ledger, so no receipt
    /// is lost to a race.
    ///
    /// # Errors
    ///
    /// [`PublicationAborted`] when the storage seam refuses the index write. The content
    /// stays behind as GC-eligible unreachable content; no index entry and no receipt are
    /// created.
    pub fn commit_index(mut self) -> Result<PublicationReceipt, PublicationAborted> {
        self.settled = true;
        let store = self.store;

        store
            .faults
            .check(PublicationPhase::CommittingIndex)
            .map_err(|reason| store.abort(PublicationPhase::CommittingIndex, reason))?;

        // Refused for `cap_*` only, which `stage` already rejected; kept as a total
        // function rather than a panic.
        let path = ArtifactPath::for_handle(&self.handle).map_err(|_| {
            store.abort(
                PublicationPhase::CommittingIndex,
                AbortReason::NotContentAddressed,
            )
        })?;

        let receipt = PublicationReceipt {
            handle: self.handle.clone(),
            actor: self.actor.clone(),
            capability: self.capability.clone(),
            cost: self.cost,
        };

        let mut state = store.state();
        state
            .index
            .entry(path)
            .or_insert_with(|| self.handle.clone());
        state
            .ledger
            .entry(self.handle.clone())
            .or_default()
            .push(receipt.clone());
        // Released explicitly rather than at end of scope: `Drop` releases this
        // publication's root pin and takes the same lock, so the guard must be gone first.
        drop(state);
        Ok(receipt)
    }

    /// Abandon after the content commit — the modeled crash between the two commits.
    pub fn abandon(mut self) -> PublicationAborted {
        self.settled = true;
        self.store
            .abort(PublicationPhase::CommittingIndex, AbortReason::Abandoned)
    }
}

impl Drop for CommittedContent<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.store
                .abort(PublicationPhase::CommittingIndex, AbortReason::Abandoned);
        }
        // Every exit from this state runs through here — receipt, abandonment, refused
        // index write, panic — so the pin taken at the content commit is released exactly
        // once and never leaks. It is released *last*: a successful publication has
        // already written the index entry that takes over as the content's root, and a
        // failed one leaves content that is now genuinely reclaimable.
        self.store.release_in_flight(&self.handle);
    }
}

// --- operator view ---------------------------------------------------------------------

/// One defect the index verifier can find, in docs/35's closed classification.
///
/// > It MUST be runnable offline against a store, MUST report results per artifact class,
/// > and MUST classify every defect it finds as exactly one of: unreachable content […];
/// > missing referent […]; identity mismatch […].
/// >
/// > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoreDefect {
    /// Content no index entry names: ordinary crash residue, or a publication caught
    /// between its two commits. Garbage-collectable once no live publication holds it —
    /// [`ReferenceStore::collect_garbage`] reclaims the first and leaves the second alone.
    UnreachableContent(ArtifactHandle),
    /// A live index entry whose content is absent. Never produced by this store's own
    /// ordering — content is committed first and stays a collection root until the index
    /// entry naming it lands — so finding one means external damage.
    MissingReferent(ArtifactPath),
    /// Content that does not identify to the identity it is filed under. Corruption:
    /// reported, never silently repaired.
    IdentityMismatch(ArtifactHandle),
}

impl fmt::Display for StoreDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreachableContent(handle) => write!(f, "unreachable content: {handle}"),
            Self::MissingReferent(path) => write!(f, "missing referent: {path}"),
            Self::IdentityMismatch(handle) => write!(f, "identity mismatch: {handle}"),
        }
    }
}

/// The privileged view of a store: receipts, attribution, and the index verifier.
///
/// Obtained from [`ReferenceStore::audit_view`] under [`Action::Audit`]. Everything here
/// discloses what the store holds, which is why it is not reachable with a read
/// capability.
#[derive(Debug)]
pub struct StoreAudit<'store> {
    store: &'store ReferenceStore,
}

impl StoreAudit<'_> {
    /// Every receipt issued for `handle`, in publication order.
    ///
    /// The ledger is append-only (plan §18.5), so this is the "no lost receipts" half of
    /// the G0-DX-13 pass condition: N publishers of one identity leave N receipts here.
    #[must_use]
    pub fn receipts(&self, handle: &ArtifactHandle) -> Vec<PublicationReceipt> {
        self.store
            .state()
            .ledger
            .get(handle)
            .cloned()
            .unwrap_or_default()
    }

    /// Every published identity, in store order.
    #[must_use]
    pub fn identities(&self) -> Vec<ArtifactHandle> {
        self.store.state().index.values().cloned().collect()
    }

    /// How many artifacts the index names.
    #[must_use]
    pub fn published_count(&self) -> usize {
        self.store.state().index.len()
    }

    /// Every abort recorded, in order (INV-017's audit trail).
    #[must_use]
    pub fn aborts(&self) -> Vec<PublicationAborted> {
        self.store.state().aborts.clone()
    }

    /// Bytes of committed content per artifact class (plan §4.5: "The daemon reports
    /// storage attribution by artifact class").
    ///
    /// Operational telemetry, deliberately *not* on a receipt (RFC 0026).
    #[must_use]
    pub fn storage_attribution(&self) -> BTreeMap<ArtifactClass, u64> {
        let state = self.store.state();
        let mut attribution = BTreeMap::new();
        for (handle, content) in &state.content {
            *attribution.entry(handle.class()).or_insert(0) +=
                u64::try_from(content.len()).unwrap_or(u64::MAX);
        }
        attribution
    }

    /// Run the index verifier, offline, over the whole store, against `declared` — the
    /// identity seam the caller asserts the store is supposed to be filed under.
    ///
    /// `declared` is deliberately not `self.store.identifier`: a store that re-derives with
    /// its own configured seam only ever checks that seam against itself, so a store built
    /// over a seam that is pure and internally consistent but is *not* the one the system
    /// declares (per ADR-0013, `Blake3Identity`) would pass its own audit with every entry
    /// misnamed. Passing the declared seam in makes that class of defect visible: content
    /// filed under an identity `declared` does not derive is [`StoreDefect::IdentityMismatch`]
    /// whether the drift is corruption or a substituted seam.
    ///
    /// Returns docs/35's three defect classes, in a deterministic order.
    #[must_use]
    pub fn fsck(&self, declared: &dyn ContentIdentifier) -> Vec<StoreDefect> {
        // Snapshot under the lock, verify outside it: the identifier is caller-supplied
        // code and must not run while the store is locked.
        let (content, index) = {
            let state = self.store.state();
            (state.content.clone(), state.index.clone())
        };

        let reachable: BTreeSet<&ArtifactHandle> = index.values().collect();
        let mut defects = Vec::new();

        for (handle, bytes) in &content {
            if !reachable.contains(handle) {
                defects.push(StoreDefect::UnreachableContent(handle.clone()));
            }
            match declared.identify(handle.class(), bytes) {
                Ok(derived) if &derived == handle => {}
                _ => defects.push(StoreDefect::IdentityMismatch(handle.clone())),
            }
        }
        for (path, handle) in &index {
            if !content.contains_key(handle) {
                defects.push(StoreDefect::MissingReferent(path.clone()));
            }
        }

        defects.sort();
        defects
    }
}

// --- helpers ---------------------------------------------------------------------------

/// Lock, tolerating poisoning.
///
/// A panic in one thread must not turn every later operation into a panic: the store's
/// state is a plain map and stays structurally intact, and a store that answers is a
/// store whose invariants can still be checked by [`StoreAudit::fsck`].
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    // --- test seams --------------------------------------------------------------------

    /// A total, deterministic content identifier for the tests.
    ///
    /// Identity is **not** this module's decision (ADR-0013, `continuum-value`, PR 2 /
    /// IMPL-03). This is the smallest pure function that lets the store's own obligations
    /// — convergence, ordering, refusal shape — be tested without importing one.
    /// `wrapping_mul` is deliberate: `overflow-checks` is on in release
    /// (`Cargo.toml`), so an unchecked `*` here would panic in half the determinism
    /// matrix.
    #[derive(Debug, Clone, Copy)]
    struct Fnv1aIdentifier;

    impl ContentIdentifier for Fnv1aIdentifier {
        fn identify(
            &self,
            class: ArtifactClass,
            content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            for byte in content {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            ArtifactHandle::new(class, &format!("{hash:016x}")).map_err(|_| IdentityUnavailable)
        }
    }

    /// Every content gets the same identity — an injected collision, so the store's
    /// ADR-0013 exact comparison can be exercised without a real hash.
    #[derive(Debug, Clone, Copy)]
    struct CollidingIdentifier;

    impl ContentIdentifier for CollidingIdentifier {
        fn identify(
            &self,
            class: ArtifactClass,
            _content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            ArtifactHandle::new(class, "collision").map_err(|_| IdentityUnavailable)
        }
    }

    /// Storage that fails at exactly one phase.
    #[derive(Debug, Clone, Copy)]
    struct FailAt(PublicationPhase);

    impl StorageFaults for FailAt {
        fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
            if phase == self.0 {
                Err(AbortReason::StorageExhausted)
            } else {
                Ok(())
            }
        }
    }

    // --- fixtures ----------------------------------------------------------------------

    const CLASS: ArtifactClass = ArtifactClass::Evidence;

    fn token(identity: &str) -> CapabilityToken {
        CapabilityToken::mint(identity).expect("test capability identity is well formed")
    }

    fn grant(identity: &str, actor: &str, level: AuthorityLevel) -> CapabilityDescriptor {
        CapabilityDescriptor::new(token(identity), ActorId::new(actor), level)
    }

    struct Fixture {
        store: ReferenceStore,
        audit: Arc<AuditLog>,
        author: CapabilityToken,
        reader: CapabilityToken,
        operator: CapabilityToken,
    }

    fn fixture() -> Fixture {
        fixture_with(Fnv1aIdentifier, NoFaults)
    }

    fn fixture_with(
        identifier: impl ContentIdentifier + 'static,
        faults: impl StorageFaults + 'static,
    ) -> Fixture {
        let audit = Arc::new(AuditLog::new());
        let store = ReferenceStore::builder(identifier, Arc::clone(&audit))
            .faults(faults)
            .capability(grant("author", "author", AuthorityLevel::Propose))
            .capability(grant("reader", "reader", AuthorityLevel::Read))
            .capability(grant("operator", "operator", AuthorityLevel::Promote))
            .build();
        Fixture {
            store,
            audit,
            author: token("author"),
            reader: token("reader"),
            operator: token("operator"),
        }
    }

    // --- vocabulary fidelity -----------------------------------------------------------

    #[test]
    fn authority_levels_are_rfc_0027_order() {
        // "read < propose < execute < revise-intent < promote"
        assert!(AuthorityLevel::Read < AuthorityLevel::Propose);
        assert!(AuthorityLevel::Propose < AuthorityLevel::Execute);
        assert!(AuthorityLevel::Execute < AuthorityLevel::ReviseIntent);
        assert!(AuthorityLevel::ReviseIntent < AuthorityLevel::Promote);
        assert_eq!(AuthorityLevel::ReviseIntent.to_string(), "revise-intent");
    }

    #[test]
    fn action_minimum_authority_follows_the_rfc_0027_registry() {
        assert_eq!(Action::Read.minimum_authority(), AuthorityLevel::Read);
        assert_eq!(Action::Publish.minimum_authority(), AuthorityLevel::Propose);
        assert_eq!(
            Action::Administer.minimum_authority(),
            AuthorityLevel::Promote
        );
        assert_eq!(Action::Audit.minimum_authority(), AuthorityLevel::Promote);
    }

    // --- the state machine -------------------------------------------------------------

    #[test]
    fn publication_walks_stage_then_content_then_index() {
        let f = fixture();
        let staged = f
            .store
            .stage(CLASS, b"payload".to_vec(), &f.author)
            .expect("authorized");
        let handle = staged.handle().clone();

        // Named before stored, and not yet visible.
        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));

        let committed = staged.commit_content().expect("content commits");
        assert_eq!(committed.handle(), &handle);
        // Content is durable, but INV-017 says visible only after the index entry.
        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));

        let receipt = committed.commit_index().expect("index commits");
        assert_eq!(receipt.handle(), &handle);
        assert_eq!(
            f.store.read(&handle, &f.reader).expect("published"),
            b"payload"
        );
    }

    #[test]
    fn a_receipt_names_the_identity_the_store_derived() {
        let f = fixture();
        let receipt = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("published");
        let expected = Fnv1aIdentifier
            .identify(CLASS, b"payload")
            .expect("identifiable");
        assert_eq!(receipt.handle(), &expected);
        assert_eq!(receipt.actor(), &ActorId::new("author"));
    }

    // --- G0-DX-13: does content addressing survive concurrency? ------------------------

    /// The G0-DX-13 pass condition, whole: "one semantic identity; no lost receipts;
    /// deterministic result" (`notes/plan/notes/G0_SPIKE_MATRIX.md`).
    ///
    /// Assertions are on outcomes only — never on interleavings, which is why this test
    /// cannot flake on a loaded machine. The [`Barrier`] maximizes contention; it does not
    /// participate in any assertion, and the assertions hold whatever order the threads
    /// happen to run in.
    #[test]
    fn concurrent_identical_publication_yields_one_identity_and_every_receipt() {
        const PUBLISHERS: usize = 16;
        const ROUNDS: usize = 8;

        let mut summaries = Vec::new();

        for _round in 0..ROUNDS {
            let audit = Arc::new(AuditLog::new());
            let mut builder = ReferenceStore::builder(Fnv1aIdentifier, Arc::clone(&audit))
                .capability(grant("operator", "operator", AuthorityLevel::Promote));
            for index in 0..PUBLISHERS {
                builder = builder.capability(grant(
                    &format!("k{index}"),
                    &format!("publisher{index}"),
                    AuthorityLevel::Propose,
                ));
            }
            let store = Arc::new(builder.build());
            let barrier = Arc::new(Barrier::new(PUBLISHERS));

            let workers: Vec<_> = (0..PUBLISHERS)
                .map(|index| {
                    let store = Arc::clone(&store);
                    let barrier = Arc::clone(&barrier);
                    let capability = token(&format!("k{index}"));
                    thread::spawn(move || {
                        barrier.wait();
                        store.publish(CLASS, b"identical bytes".to_vec(), &capability)
                    })
                })
                .collect();

            let receipts: Vec<PublicationReceipt> = workers
                .into_iter()
                .map(|worker| worker.join().expect("no publisher panicked"))
                .map(|result| result.expect("every publication succeeds"))
                .collect();

            // One semantic identity: every publisher got the same handle.
            let identities: BTreeSet<&ArtifactHandle> =
                receipts.iter().map(PublicationReceipt::handle).collect();
            assert_eq!(identities.len(), 1, "publishers disagreed on identity");
            let identity = receipts[0].handle().clone();

            // ... and the store holds exactly one artifact.
            let view = store.audit_view(&token("operator")).expect("operator");
            assert_eq!(view.published_count(), 1);
            assert_eq!(view.identities(), vec![identity.clone()]);

            // No lost receipts: one per publisher, every publisher distinct.
            let ledger = view.receipts(&identity);
            assert_eq!(ledger.len(), PUBLISHERS);
            let actors: BTreeSet<&ActorId> = ledger.iter().map(PublicationReceipt::actor).collect();
            assert_eq!(actors.len(), PUBLISHERS);

            // Convergence is invisible in cost (RFC 0026 existence-oracle rule).
            let costs: BTreeSet<PublicationCost> =
                ledger.iter().map(PublicationReceipt::cost).collect();
            assert_eq!(costs.len(), 1);
            assert_eq!(
                costs.into_iter().next().expect("one cost"),
                PublicationCost::for_content(b"identical bytes")
            );

            // Nothing was left half-done and nothing aborted.
            assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
            assert_eq!(view.aborts(), Vec::new());

            // Every publisher authorized exactly once, and every decision was audited.
            assert_eq!(
                audit
                    .records()
                    .iter()
                    .filter(|record| record.action() == Action::Publish)
                    .count(),
                PUBLISHERS
            );

            summaries.push(summarize(&view, &identity));
        }

        // Deterministic result: the store state is identical in every round, so the
        // outcome is a function of the inputs and not of the schedule.
        assert!(
            summaries.windows(2).all(|pair| pair[0] == pair[1]),
            "publication outcome varied across rounds: {summaries:#?}"
        );
    }

    /// A schedule-independent description of a finished store.
    ///
    /// Receipt *arrival order* is genuinely nondeterministic — the ledger is an
    /// append-only log (plan §18.5) and records who arrived when. The receipt *multiset*
    /// is not, so it is sorted here: what must be deterministic is what the store holds,
    /// not the order threads reached it.
    fn summarize(view: &StoreAudit<'_>, identity: &ArtifactHandle) -> String {
        let mut receipts = view.receipts(identity);
        receipts.sort();
        format!(
            "identities={:?} attribution={:?} defects={:?} aborts={:?} receipts={:?}",
            view.identities(),
            view.storage_attribution(),
            view.fsck(&Fnv1aIdentifier),
            view.aborts(),
            receipts
        )
    }

    #[test]
    fn distinct_artifacts_never_share_identity() {
        let f = fixture();
        let contents: Vec<Vec<u8>> = (0..32u8)
            .map(|n| vec![n, n.wrapping_add(7), 0xaa])
            .collect();

        let handles: Vec<ArtifactHandle> = contents
            .iter()
            .map(|content| {
                f.store
                    .publish(CLASS, content.clone(), &f.author)
                    .expect("published")
                    .handle()
                    .clone()
            })
            .collect();

        let distinct: BTreeSet<&ArtifactHandle> = handles.iter().collect();
        assert_eq!(distinct.len(), contents.len());

        let paths: BTreeSet<ArtifactPath> = handles
            .iter()
            .map(|handle| ArtifactPath::for_handle(handle).expect("content-addressed"))
            .collect();
        assert_eq!(paths.len(), contents.len());

        for (handle, content) in handles.iter().zip(&contents) {
            assert_eq!(
                &f.store.read(handle, &f.reader).expect("published"),
                content
            );
        }

        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), contents.len());
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }

    #[test]
    fn concurrent_distinct_publications_stay_distinct() {
        const PUBLISHERS: usize = 12;

        let audit = Arc::new(AuditLog::new());
        let mut builder = ReferenceStore::builder(Fnv1aIdentifier, Arc::clone(&audit))
            .capability(grant("operator", "operator", AuthorityLevel::Promote));
        for index in 0..PUBLISHERS {
            builder = builder.capability(grant(
                &format!("k{index}"),
                &format!("publisher{index}"),
                AuthorityLevel::Propose,
            ));
        }
        let store = Arc::new(builder.build());
        let barrier = Arc::new(Barrier::new(PUBLISHERS));

        let workers: Vec<_> = (0..PUBLISHERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let capability = token(&format!("k{index}"));
                thread::spawn(move || {
                    barrier.wait();
                    store.publish(CLASS, format!("payload {index}").into_bytes(), &capability)
                })
            })
            .collect();

        let handles: BTreeSet<ArtifactHandle> = workers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .map(|result| result.expect("published").handle().clone())
            .collect();

        assert_eq!(handles.len(), PUBLISHERS);
        let view = store.audit_view(&token("operator")).expect("operator");
        assert_eq!(view.published_count(), PUBLISHERS);
        for handle in &handles {
            assert_eq!(view.receipts(handle).len(), 1);
        }
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }

    #[test]
    fn a_second_publication_of_identical_content_converges_without_a_second_entry() {
        let f = fixture();
        let first = f
            .store
            .publish(CLASS, b"same".to_vec(), &f.author)
            .expect("published");

        let second_author = f
            .store
            .mint(
                &f.operator,
                grant("second", "second", AuthorityLevel::Propose),
            )
            .expect("operator may mint");
        let second = f
            .store
            .publish(CLASS, b"same".to_vec(), &second_author)
            .expect("published");

        assert_eq!(first.handle(), second.handle());
        // RFC 0026: same envelope, same cost, whoever got there first.
        assert_eq!(first.cost(), second.cost());

        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), 1);
        assert_eq!(view.receipts(first.handle()).len(), 2);
        assert_eq!(
            view.storage_attribution(),
            BTreeMap::from([(CLASS, 4)]),
            "converged content is stored once"
        );
    }

    // --- ADR-0013: exact comparison, store side ----------------------------------------

    #[test]
    fn an_identity_collision_aborts_rather_than_conflating_two_artifacts() {
        let f = fixture_with(CollidingIdentifier, NoFaults);
        let first = f
            .store
            .publish(CLASS, b"original".to_vec(), &f.author)
            .expect("published");

        let refusal = f
            .store
            .publish(CLASS, b"impostor".to_vec(), &f.author)
            .expect_err("a colliding publication must not succeed");
        assert_eq!(
            refusal,
            PublishRefusal::Aborted(PublicationAborted::new(
                PublicationPhase::CommittingContent,
                AbortReason::IdentityCollision,
            ))
        );

        // The first artifact is untouched: no overwrite, no truncation.
        assert_eq!(
            f.store.read(first.handle(), &f.reader).expect("published"),
            b"original"
        );
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.receipts(first.handle()).len(), 1);
        assert_eq!(view.published_count(), 1);
    }

    // --- abort and crash semantics -----------------------------------------------------

    #[test]
    fn an_abort_before_the_content_commit_leaves_nothing_at_all() {
        let f = fixture_with(Fnv1aIdentifier, FailAt(PublicationPhase::CommittingContent));
        let handle = Fnv1aIdentifier
            .identify(CLASS, b"payload")
            .expect("identifiable");

        let refusal = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect_err("storage refused");
        assert_eq!(
            refusal,
            PublishRefusal::Aborted(PublicationAborted::new(
                PublicationPhase::CommittingContent,
                AbortReason::StorageExhausted,
            ))
        );

        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), 0);
        assert!(view.receipts(&handle).is_empty());
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.storage_attribution(), BTreeMap::new());
        assert_eq!(view.aborts().len(), 1);
    }

    #[test]
    fn a_crash_between_the_commits_leaves_unreachable_content_and_never_an_index_entry() {
        let f = fixture_with(Fnv1aIdentifier, FailAt(PublicationPhase::CommittingIndex));
        let handle = Fnv1aIdentifier
            .identify(CLASS, b"payload")
            .expect("identifiable");

        let refusal = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect_err("storage refused the index write");
        assert_eq!(
            refusal,
            PublishRefusal::Aborted(PublicationAborted::new(
                PublicationPhase::CommittingIndex,
                AbortReason::StorageExhausted,
            ))
        );

        // Nothing observable was published: no read, no receipt, no index entry.
        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), 0);
        assert!(view.receipts(&handle).is_empty());

        // What is left is exactly docs/35's crash residue, classified and reclaimable.
        assert_eq!(
            view.fsck(&Fnv1aIdentifier),
            vec![StoreDefect::UnreachableContent(handle.clone())]
        );

        let reclaimed = f
            .store
            .collect_garbage(&f.operator)
            .expect("operator may collect");
        assert_eq!(reclaimed, vec![handle]);
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.storage_attribution(), BTreeMap::new());
    }

    #[test]
    fn dropping_a_committed_content_is_the_same_crash() {
        let f = fixture();
        let handle = {
            let committed = f
                .store
                .stage(CLASS, b"payload".to_vec(), &f.author)
                .expect("authorized")
                .commit_content()
                .expect("content commits");
            let handle = committed.handle().clone();
            drop(committed);
            handle
        };

        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), 0);
        assert!(view.receipts(&handle).is_empty());
        assert_eq!(
            view.fsck(&Fnv1aIdentifier),
            vec![StoreDefect::UnreachableContent(handle)]
        );
        assert_eq!(
            view.aborts(),
            vec![PublicationAborted::new(
                PublicationPhase::CommittingIndex,
                AbortReason::Abandoned
            )]
        );
    }

    #[test]
    fn dropping_a_staged_publication_leaves_the_store_untouched() {
        let f = fixture();
        let handle = {
            let staged = f
                .store
                .stage(CLASS, b"payload".to_vec(), &f.author)
                .expect("authorized");
            staged.handle().clone()
        };

        assert_eq!(f.store.read(&handle, &f.reader), Err(CapabilityDenied));
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.published_count(), 0);
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.storage_attribution(), BTreeMap::new());
        assert_eq!(
            view.aborts(),
            vec![PublicationAborted::new(
                PublicationPhase::Staging,
                AbortReason::Abandoned
            )]
        );
    }

    #[test]
    fn garbage_collection_never_reclaims_a_publication_still_in_flight() {
        // G0-DX-13, bn-21dd → bn-2siid. A root set derived from the index alone calls a
        // publication between its two commits unreachable and reclaims content the store
        // has already told the publisher is durable. Stated single-threaded: the race in
        // `tests/dx13_falsification.rs` only makes the same window easier to hit.
        let f = fixture();
        let committed = f
            .store
            .stage(CLASS, b"payload".to_vec(), &f.author)
            .expect("authorized")
            .commit_content()
            .expect("content commits");

        let reclaimed = f
            .store
            .collect_garbage(&f.operator)
            .expect("operator may collect");
        assert_eq!(
            reclaimed,
            Vec::new(),
            "collection reclaimed content a live publication is holding"
        );

        let receipt = committed.commit_index().expect("the index commits");
        assert_eq!(
            f.store
                .read(receipt.handle(), &f.reader)
                .expect("published"),
            b"payload",
            "a receipt was issued for an artifact the store cannot read back"
        );
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }

    #[test]
    fn an_in_flight_pin_is_released_only_when_its_last_holder_settles() {
        // Convergent publishers share an identity, so the root is held by a count and not
        // a flag: the first one to settle must not release the second one's claim.
        let f = fixture();
        let first = f
            .store
            .stage(CLASS, b"payload".to_vec(), &f.author)
            .expect("authorized")
            .commit_content()
            .expect("content commits");
        let second = f
            .store
            .stage(CLASS, b"payload".to_vec(), &f.author)
            .expect("authorized")
            .commit_content()
            .expect("the second publication converges onto the same content");
        let handle = first.handle().clone();
        assert_eq!(second.handle(), &handle, "convergence derives one identity");

        drop(first);
        assert_eq!(
            f.store
                .collect_garbage(&f.operator)
                .expect("operator may collect"),
            Vec::new(),
            "one publication settling released a root the other still holds"
        );

        drop(second);
        assert_eq!(
            f.store
                .collect_garbage(&f.operator)
                .expect("operator may collect"),
            vec![handle],
            "the pin outlived every publication holding it: residue is not reclaimable"
        );
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.storage_attribution(), BTreeMap::new());
    }

    #[test]
    fn a_retry_after_an_abort_publishes_cleanly() {
        // RFC 0026: "the client MAY retry […] and the retry is a fresh publication, not a
        // resumption of a partial one".
        let f = fixture();
        let aborted = f
            .store
            .stage(CLASS, b"payload".to_vec(), &f.author)
            .expect("authorized")
            .abandon();
        assert_eq!(aborted.reason(), AbortReason::Abandoned);

        let receipt = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("retry publishes");
        assert_eq!(
            f.store
                .read(receipt.handle(), &f.reader)
                .expect("published"),
            b"payload"
        );
        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(view.receipts(receipt.handle()).len(), 1);
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }

    // --- possession-independent authorization ------------------------------------------

    #[test]
    fn capability_denied_carries_zero_bits() {
        // The structural half of the existence-oracle rule: a unit struct has exactly one
        // value, so the refusal *cannot* vary with what the store holds.
        assert_eq!(core::mem::size_of::<CapabilityDenied>(), 0);
        assert_eq!(CapabilityDenied, CapabilityDenied);
        assert_eq!(CapabilityDenied.to_string(), "capability denied");
    }

    #[test]
    fn a_denied_read_is_identical_for_a_present_and_an_absent_artifact() {
        let f = fixture();
        let present = f
            .store
            .publish(CLASS, b"secret".to_vec(), &f.author)
            .expect("published")
            .handle()
            .clone();
        let absent = Fnv1aIdentifier
            .identify(CLASS, b"never published")
            .expect("identifiable");

        let outsider = token("outsider"); // never registered: a leaked-handle attacker
        let on_present = f.store.read(&present, &outsider);
        let on_absent = f.store.read(&absent, &outsider);

        assert_eq!(on_present, Err(CapabilityDenied));
        assert_eq!(on_absent, Err(CapabilityDenied));
        assert_eq!(on_present, on_absent);
        assert_eq!(format!("{on_present:?}"), format!("{on_absent:?}"));
        assert_eq!(
            on_present.unwrap_err().to_string(),
            on_absent.unwrap_err().to_string()
        );
    }

    #[test]
    fn an_authorized_read_of_an_absent_artifact_is_also_capability_denied() {
        let f = fixture();
        let absent = Fnv1aIdentifier
            .identify(CLASS, b"never published")
            .expect("identifiable");
        // The reader is fully authorized for this class. Fail-closed keeps the refusal
        // uniform, so a permitted caller cannot be used as the oracle either.
        assert_eq!(f.store.read(&absent, &f.reader), Err(CapabilityDenied));
    }

    #[test]
    fn the_same_denied_request_audits_identically_before_and_after_publication() {
        // The audit log must not become the oracle the refusal is not: the record is
        // written at decision time and names no outcome (plan §18.5).
        let f = fixture();
        let handle = Fnv1aIdentifier
            .identify(CLASS, b"payload")
            .expect("identifiable");
        let outsider = token("outsider");

        assert_eq!(f.store.read(&handle, &outsider), Err(CapabilityDenied));
        let before = f.audit.records().last().cloned().expect("audited");

        f.store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("published");

        assert_eq!(f.store.read(&handle, &outsider), Err(CapabilityDenied));
        let after = f.audit.records().last().cloned().expect("audited");

        assert_eq!(before, after);
        assert_eq!(before.decision(), AuthorizationDecision::Denied);
        assert_eq!(before.actor(), None);
    }

    #[test]
    fn a_leaked_handle_without_a_capability_opens_nothing() {
        // The bone's second acceptance criterion, and ADR-0037's rule.
        let f = fixture();
        let receipt = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("published");
        let leaked = receipt.handle().clone();

        let unregistered = token("stolen");
        assert_eq!(f.store.read(&leaked, &unregistered), Err(CapabilityDenied));
        assert_eq!(
            f.store.publish(CLASS, b"other".to_vec(), &unregistered),
            Err(PublishRefusal::CapabilityDenied(CapabilityDenied))
        );
        assert_eq!(
            f.store.audit_view(&unregistered).err(),
            Some(CapabilityDenied)
        );

        // Nor can the leaked handle be laundered into a capability.
        assert_eq!(
            CapabilityToken::from_handle(leaked).expect_err("a name is not a capability"),
            InvalidCapabilityToken::NotACapabilityClass(CLASS)
        );
    }

    #[test]
    fn a_read_capability_cannot_publish_and_a_publish_capability_cannot_audit() {
        let f = fixture();
        assert_eq!(
            f.store.publish(CLASS, b"payload".to_vec(), &f.reader),
            Err(PublishRefusal::CapabilityDenied(CapabilityDenied))
        );
        assert_eq!(f.store.audit_view(&f.author).err(), Some(CapabilityDenied));
        assert_eq!(f.store.audit_view(&f.reader).err(), Some(CapabilityDenied));
        assert_eq!(
            f.store.collect_garbage(&f.author).err(),
            Some(CapabilityDenied)
        );
    }

    #[test]
    fn a_capability_scoped_to_another_class_is_denied() {
        let audit = Arc::new(AuditLog::new());
        let store = ReferenceStore::builder(Fnv1aIdentifier, Arc::clone(&audit))
            .capability(grant("author", "author", AuthorityLevel::Propose))
            .capability(
                CapabilityDescriptor::new(
                    token("narrow"),
                    ActorId::new("narrow"),
                    AuthorityLevel::Read,
                )
                .scoped_to([ArtifactClass::ProofArtifact]),
            )
            .build();

        let receipt = store
            .publish(CLASS, b"payload".to_vec(), &token("author"))
            .expect("published");
        assert_eq!(
            store.read(receipt.handle(), &token("narrow")),
            Err(CapabilityDenied)
        );
    }

    #[test]
    fn a_revoked_capability_stops_working() {
        let f = fixture();
        let receipt = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("published");
        assert!(f.store.read(receipt.handle(), &f.reader).is_ok());

        f.store.revoke(&f.operator, &f.reader).expect("operator");
        assert_eq!(
            f.store.read(receipt.handle(), &f.reader),
            Err(CapabilityDenied)
        );
    }

    #[test]
    fn minting_cannot_escalate_above_the_minting_capability() {
        let f = fixture();
        // An author may not mint at all: minting is `promote`.
        assert_eq!(
            f.store
                .mint(&f.author, grant("new", "new", AuthorityLevel::Read))
                .err(),
            Some(CapabilityDenied)
        );

        // An operator may mint at or below its own level, and no higher.
        let delegated = f
            .store
            .mint(&f.operator, grant("mid", "mid", AuthorityLevel::Execute))
            .expect("operator may delegate downward");
        assert_eq!(
            f.store
                .mint(&delegated, grant("up", "up", AuthorityLevel::Promote))
                .err(),
            Some(CapabilityDenied)
        );
    }

    #[test]
    fn capability_tokens_are_minted_and_never_published() {
        let f = fixture();
        // docs/35: `cap_*` is outside the storage lifecycle entirely, and
        // `ArtifactPath::for_handle` refuses to give it a path.
        assert_eq!(
            f.store
                .publish(ArtifactClass::Capability, b"s3cret".to_vec(), &f.author),
            Err(PublishRefusal::CapabilityDenied(CapabilityDenied))
        );
    }

    #[test]
    fn a_capability_token_never_prints_its_secret() {
        // RFC 0026: tokens "MUST NOT be logged in request traces, MUST NOT appear in
        // error text".
        let secret = token("hunter2");
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("hunter2"), "{rendered}");

        let record = AuthorizationRecord {
            actor: None,
            capability: secret,
            action: Action::Read,
            class: Some(CLASS),
            handle: None,
            decision: AuthorizationDecision::Denied,
        };
        assert!(!format!("{record:?}").contains("hunter2"));
        assert!(!CapabilityDenied.to_string().contains("hunter2"));
    }

    // --- audit seam --------------------------------------------------------------------

    #[test]
    fn every_authorization_decision_is_audited() {
        let f = fixture();
        let receipt = f
            .store
            .publish(CLASS, b"payload".to_vec(), &f.author)
            .expect("published");
        f.store.read(receipt.handle(), &f.reader).expect("read");
        assert_eq!(
            f.store.read(receipt.handle(), &token("outsider")),
            Err(CapabilityDenied)
        );

        let records = f.audit.records();
        assert_eq!(records.len(), 3);

        assert_eq!(records[0].action(), Action::Publish);
        assert_eq!(records[0].actor(), Some(&ActorId::new("author")));
        assert_eq!(records[0].class(), Some(CLASS));
        assert_eq!(records[0].handle(), None); // authorized before the identity exists
        assert_eq!(records[0].decision(), AuthorizationDecision::Permitted);

        assert_eq!(records[1].action(), Action::Read);
        assert_eq!(records[1].actor(), Some(&ActorId::new("reader")));
        assert_eq!(records[1].handle(), Some(receipt.handle()));
        assert_eq!(records[1].decision(), AuthorizationDecision::Permitted);

        assert_eq!(records[2].decision(), AuthorizationDecision::Denied);
        assert_eq!(records[2].capability(), &token("outsider"));
    }

    #[test]
    fn a_custom_policy_sees_no_store_state() {
        /// A policy that permits everything it is asked about. It still cannot leak
        /// existence, because `AccessRequest` carries no way to learn it — the point of
        /// this test is that the trait's signature admits no such implementation.
        struct PermitAll;
        impl AuthorizationPolicy for PermitAll {
            fn decide(&self, request: &AccessRequest<'_>) -> AuthorizationDecision {
                assert!(request.descriptor().capability() == request.capability());
                AuthorizationDecision::Permitted
            }
        }

        let audit = Arc::new(AuditLog::new());
        let store = ReferenceStore::builder(Fnv1aIdentifier, Arc::clone(&audit))
            .policy(PermitAll)
            .capability(grant("any", "any", AuthorityLevel::Read))
            .build();

        // Permissive policy, registered token: publication is allowed.
        let receipt = store
            .publish(CLASS, b"payload".to_vec(), &token("any"))
            .expect("permitted");
        assert_eq!(
            store.read(receipt.handle(), &token("any")).expect("read"),
            b"payload"
        );

        // An unregistered token is still refused: authentication precedes policy, so a
        // permissive policy cannot resurrect a token the store never minted.
        assert_eq!(
            store.read(receipt.handle(), &token("ghost")),
            Err(CapabilityDenied)
        );
    }

    // --- attribution -------------------------------------------------------------------

    #[test]
    fn storage_attribution_is_reported_per_artifact_class() {
        // plan §4.5: "The daemon reports storage attribution by artifact class."
        let f = fixture();
        f.store
            .publish(ArtifactClass::Evidence, b"1234".to_vec(), &f.author)
            .expect("published");
        f.store
            .publish(ArtifactClass::ProofArtifact, b"123456".to_vec(), &f.author)
            .expect("published");

        let view = f.store.audit_view(&f.operator).expect("operator");
        assert_eq!(
            view.storage_attribution(),
            BTreeMap::from([
                (ArtifactClass::Evidence, 4),
                (ArtifactClass::ProofArtifact, 6)
            ])
        );
    }

    // --- Published<H> (bn-283p6) ------------------------------------------------------

    /// A receipt attests its own handle, and refuses every other name: a different
    /// identity, and the same identity under a different class.
    #[test]
    fn published_attests_only_the_receipts_own_handle() {
        let f = fixture();
        let receipt = f
            .store
            .publish(CLASS, b"published".to_vec(), &f.author)
            .expect("the author publishes");
        let own = Published::attest(&receipt, receipt.handle().clone()).expect("its own handle");
        assert_eq!(own.handle(), receipt.handle());
        assert_eq!(own, Published::of(&receipt));

        let other = f
            .store
            .publish(CLASS, b"another".to_vec(), &f.author)
            .expect("the author publishes");
        assert_eq!(
            Published::attest(&receipt, other.handle().clone()),
            Err(NameNotPublished),
            "a receipt for one artifact never attests another"
        );
        let text = receipt.handle().to_string();
        let identity = text.strip_prefix(CLASS.prefix()).expect("the class prefix");
        let reclassed =
            ArtifactHandle::new(ArtifactClass::Task, identity).expect("a well-formed handle");
        assert_eq!(
            Published::attest(&receipt, reclassed),
            Err(NameNotPublished),
            "nor the same identity under another class"
        );
        assert_eq!(own.into_handle(), receipt.handle().clone());
    }

    /// **Metamorphic relation: serialization round trip.** A handle carried through its
    /// wire spelling and parsed back is attested exactly as the receipt's own handle is:
    /// `attest(r, parse(display(h)))` equals `of(r)` for every published `h`.
    #[test]
    fn published_is_invariant_under_the_handles_serialization_round_trip() {
        let f = fixture();
        for content in [&b"a"[..], b"bb", b"", b"\x00\xff"] {
            let receipt = f
                .store
                .publish(CLASS, content.to_vec(), &f.author)
                .expect("the author publishes");
            let round_tripped: ArtifactHandle = receipt
                .handle()
                .to_string()
                .parse()
                .expect("a handle parses from its own spelling");
            assert_eq!(
                Published::attest(&receipt, round_tripped),
                Ok(Published::of(&receipt)),
                "serialization round trip"
            );
        }
    }
}
