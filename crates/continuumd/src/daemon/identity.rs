//! The two identity seams the operation layer composes, and the handle conversions
//! between the wire spelling and the store's.
//!
//! # Why the daemon is where these meet
//!
//! Neither seam is invented here. `continuum-value` owns *what an identity is* (ADR-0013)
//! and `continuum-workspace` owns *where an artifact goes* and *when it becomes visible*:
//!
//! > Deriving the path is not owning the identity: computing content identities is PR 2's
//! > `continuum-value`, and durably publishing under one is PR 6+ and `continuumd`.
//! >
//! > — `crates/continuum-workspace/src/lib.rs`
//!
//! So the composition — hash the canonical bytes, spell the digest as the identity half of
//! a plan §4.4 handle — is the daemon's, and [`Blake3Identity`] is it. The spelling is not
//! a choice this module makes either:
//!
//! > Lowercase hex is inside `[A-Za-z0-9_-]`, the identity character class
//! > `crates/continuum-workspace/src/artifact_path.rs` accepts, so this token is directly
//! > usable as the identity half of a plan §4.4 artifact handle.
//! >
//! > — `continuum_value::identity::Digest256::to_token`
//!
//! # Handles cross a boundary and are checked crossing it
//!
//! The wire has nineteen handle *types*, one per class, each enforcing its own prefix; the
//! store has one [`ArtifactHandle`] carrying an [`ArtifactClass`]. The conversion is
//! total in neither direction — a wire `WorkspaceHandle` is `ws_` followed by
//! `[A-Za-z0-9_-]+`, and the store additionally forbids an empty identity — so every
//! crossing goes through the functions here and reports [`HandleMismatch`] rather than
//! panicking. A daemon that unwrapped a handle conversion would abort on a value a client
//! chose.

use core::fmt;

use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};

use crate::protocol::scalar::{
    ActorId, AuditCorrelationId, CapabilityHandle, DiffHandle, IntentHandle, RequestId,
    WorkspaceHandle,
};

/// The production content identity: BLAKE3 over the canonical record, spelled as the
/// identity half of a plan §4.4 handle.
///
/// Pure in its arguments and stateless, as [`ContentIdentifier`] requires: "two calls with
/// equal arguments, in any process, must return equal handles (INV-005, INV-006)". No
/// clock, no seed, no interior state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Blake3Identity;

impl ContentIdentifier for Blake3Identity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &Blake3Hasher::hash(content).to_token())
            .map_err(|_| IdentityUnavailable)
    }
}

/// Derives the audit-correlation identity a result cites.
///
/// A seam rather than a free function because `rule audit.correlation` constrains the
/// *function*, not the algorithm: the identity "MUST be a function of the request identity
/// alone — `request_id` and `actor` — and MUST NOT vary with the outcome, the decision, the
/// presented capability, or whether a named artifact exists", and "a per-record random
/// identity is prohibited". Any implementation satisfying that is admissible; the daemon
/// ships [`HashedCorrelation`].
pub trait AuditCorrelator: Send + Sync {
    /// The correlation identity for a request identity.
    ///
    /// Implementations MUST be pure in their two arguments. They are given no capability,
    /// no outcome, and no store, so `rule audit.correlation`'s prohibitions are properties
    /// of this signature rather than of implementer discipline.
    fn correlate(&self, request_id: &RequestId, actor: &ActorId) -> AuditCorrelationId;
}

/// The daemon's correlator: BLAKE3 over an unambiguous encoding of the request identity.
///
/// The preimage is `len(request_id) | request_id | len(actor) | actor`, both lengths as
/// eight big-endian bytes, so no pair of identities can produce another pair's preimage by
/// concatenation. The digest's lowercase-hex token satisfies `AuditCorrelationId`'s
/// `^[A-Za-z0-9_-]+$` by construction.
///
/// Hashing rather than concatenating is not obfuscation — the identity is emitted to the
/// caller, who supplied both halves — it is what makes the value a fixed-width token
/// inside the declared pattern when `ActorId` is not: an actor is
/// `^(agent|human|service|ci):[A-Za-z0-9._:-]+$`, whose `:` and `.` are outside
/// `AuditCorrelationId`'s class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HashedCorrelation;

impl AuditCorrelator for HashedCorrelation {
    fn correlate(&self, request_id: &RequestId, actor: &ActorId) -> AuditCorrelationId {
        let mut preimage = Vec::new();
        for part in [request_id.as_str(), actor.as_str()] {
            preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
            preimage.extend_from_slice(part.as_bytes());
        }
        AuditCorrelationId::new(&Blake3Hasher::hash(&preimage).to_token())
            .expect("a lowercase-hex digest token is inside `^[A-Za-z0-9_-]+$`")
    }
}

/// A handle that does not cross between the wire spelling and the store's.
///
/// [`fmt::Display`] names the class and never the offending text: the text came off the
/// wire, and a typed error's detail is "stable, non-interpolated" (INV-016,
/// `rule envelope.no_prose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleMismatch {
    /// The artifact class the handle was offered as.
    pub class: ArtifactClass,
}

impl fmt::Display for HandleMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the offered handle is not a well-formed `{}` identity",
            self.class
        )
    }
}

impl core::error::Error for HandleMismatch {}

/// The store handle a wire [`WorkspaceHandle`] names.
///
/// # Errors
///
/// [`HandleMismatch`] when the identity half is empty or carries a character outside
/// `[A-Za-z0-9_-]`, which the store's class admits and the wire's `ws_` prefix rule does
/// not fully imply.
pub fn workspace_to_store(handle: &WorkspaceHandle) -> Result<ArtifactHandle, HandleMismatch> {
    parse(ArtifactClass::WorkspaceSnapshot, handle.as_str())
}

/// The wire [`WorkspaceHandle`] a store handle names.
///
/// # Errors
///
/// [`HandleMismatch`] when the store handle is not of the workspace-snapshot class.
pub fn workspace_to_wire(handle: &ArtifactHandle) -> Result<WorkspaceHandle, HandleMismatch> {
    render(
        ArtifactClass::WorkspaceSnapshot,
        handle,
        WorkspaceHandle::new,
    )
}

/// The store handle a wire [`IntentHandle`] names.
///
/// # Errors
///
/// [`HandleMismatch`], as [`workspace_to_store`].
pub fn intent_to_store(handle: &IntentHandle) -> Result<ArtifactHandle, HandleMismatch> {
    parse(ArtifactClass::IntentContract, handle.as_str())
}

/// The wire [`IntentHandle`] a store handle names.
///
/// # Errors
///
/// [`HandleMismatch`] when the store handle is not of the intent-contract class.
pub fn intent_to_wire(handle: &ArtifactHandle) -> Result<IntentHandle, HandleMismatch> {
    render(ArtifactClass::IntentContract, handle, IntentHandle::new)
}

/// The wire [`DiffHandle`] a store handle names.
///
/// # Errors
///
/// [`HandleMismatch`] when the store handle is not of the diff class.
pub fn diff_to_wire(handle: &ArtifactHandle) -> Result<DiffHandle, HandleMismatch> {
    render(ArtifactClass::Diff, handle, DiffHandle::new)
}

/// The store token a wire [`CapabilityHandle`] names.
///
/// A conversion, never an authorization: the store's own
/// [`CapabilityToken::from_handle`](continuum_workspace::publication::CapabilityToken::from_handle)
/// refuses every content-addressed class, so a `ws_` handle cannot be laundered into a
/// capability through this function either.
///
/// # Errors
///
/// [`HandleMismatch`], as [`workspace_to_store`].
pub fn capability_to_store(
    handle: &CapabilityHandle,
) -> Result<continuum_workspace::publication::CapabilityToken, HandleMismatch> {
    let stored = parse(ArtifactClass::Capability, handle.as_str())?;
    continuum_workspace::publication::CapabilityToken::from_handle(stored).map_err(|_| {
        HandleMismatch {
            class: ArtifactClass::Capability,
        }
    })
}

/// The store's actor name for a wire [`ActorId`].
///
/// Total: the store's `ActorId` is an unconstrained name for attribution, and the wire's is
/// a strictly narrower closed-scheme spelling, so every wire actor is a store actor and no
/// conversion can fail. The reverse direction deliberately has no function — a store name
/// is not necessarily a wire actor, and inventing a scheme for one would be inventing an
/// identity.
#[must_use]
pub fn actor_to_store(actor: &ActorId) -> continuum_workspace::publication::ActorId {
    continuum_workspace::publication::ActorId::new(actor.as_str())
}

fn parse(class: ArtifactClass, text: &str) -> Result<ArtifactHandle, HandleMismatch> {
    let identity = text
        .strip_prefix(class.prefix())
        .ok_or(HandleMismatch { class })?;
    ArtifactHandle::new(class, identity).map_err(|_| HandleMismatch { class })
}

fn render<T, E>(
    class: ArtifactClass,
    handle: &ArtifactHandle,
    build: impl Fn(&str) -> Result<T, E>,
) -> Result<T, HandleMismatch> {
    if handle.class() != class {
        return Err(HandleMismatch { class });
    }
    build(&handle.to_string()).map_err(|_| HandleMismatch { class })
}
