//! Stale-snapshot error: supersession-aware staleness as a typed, testable property
//! (plan §4.2, PR 3 / IMPL-04).
//!
//! # What this module decides
//!
//! > An old snapshot remains reproducible after the working tree changes; using a stale one
//! > is a typed error, never a silent re-read.
//! >
//! > — `lib.rs`, restating plan §4.2's exit criterion
//!
//! [`lineage`](crate::lineage) says what a [`Fork`] *is*: a divergence point (`origin`), the
//! head it most recently replaced (`parent`), and its current head. This module says what it
//! *means* to hand one of those three identities — or any other — to an operation that only
//! the current head may answer for: the current head is fine, an identity that really was a
//! point in this lineage but is not the head anymore is [`StaleSnapshot`], and an identity
//! this lineage cannot place at all is the distinct [`UnknownSnapshot`] (INV-008 — the two
//! outcomes are never collapsed).
//!
//! # Not [`BaseMismatch`](crate::diff::BaseMismatch)
//!
//! `diff.rs` draws this line already and it is repeated here because the two are easy to
//! confuse: [`BaseMismatch`](crate::diff::BaseMismatch) is "these two values were never
//! related" — a derived snapshot diffed against a base it was never derived from, where every
//! reported change would be meaningless from the start. Staleness is the opposite shape: the
//! identity *was* related to this lineage — it is the lineage's own origin or a head it once
//! had — and the question is only whether the world has moved on since. A caller with a
//! [`BaseMismatch`](crate::diff::BaseMismatch) has the wrong value entirely; a caller with a
//! [`StaleSnapshot`] has the right lineage and a stale coordinate in it, which is exactly why
//! the error carries the current head's identity to redo the step against, and
//! `BaseMismatch` carries nothing to redo anything with.
//!
//! # Wire alignment
//!
//! This crate takes no protocol dependency (`lib.rs`'s dependency-boundary contract), so
//! nothing here names `ErrorCode`, `NextOperation`, or any IDL type. What it does is answer,
//! ahead of time, the question PR 5's daemon will have to answer on the wire for
//! `ErrorCode::StaleSnapshot`
//! (`notes/plan/schemas/continuumd-native-protocol.idl`): *"The named snapshot is not the
//! current one, or is not sealed where sealing is required."* This module's half is the first
//! clause — currency within a lineage the daemon already has a [`Fork`] for; the second clause
//! (an input that has not been durably sealed at all) is a durability question this crate does
//! not have an opinion on, since every [`Snapshot`] value here is already immutable in memory
//! regardless of whether [`seal`](crate::seal) has published it anywhere.
//!
//! Three normative sources agree on the shape a caller gets back, and [`StaleSnapshot`] is
//! built to match:
//!
//! - `rule errors.common` (the IDL): *"if it takes a non-null `snapshot`: `StaleSnapshot`"* —
//!   staleness is checked wherever a snapshot identity is *named*, which is exactly
//!   [`check_current`]'s signature: a [`Fork`] plus a named [`ArtifactHandle`].
//! - RFC 0026's error taxonomy: `StaleSnapshot` is class `input`, and an identical retry
//!   cannot succeed — *"no — re-seal or re-base"*. Its "Resume decision table" gives the
//!   concrete condition this module implements one level down: *"the pinned snapshot is no
//!   longer the current sealed snapshot"*.
//! - RFC 0027's H8: *"a stale-handle failure is recoverable and says so"* — every stale
//!   result carries a `recovery` list a client can execute, never a bare refusal. This crate
//!   has no wire `recovery` field to fill in, so it does the equivalent the value layer can
//!   do: [`StaleSnapshot::current`] names exactly what [`Fork::advance`]/[`Fork::advance_to`]
//!   need to redo the step against. The caller does not need the error to hand back a `Fork`
//!   to act on this — it is the same value the caller passed to [`check_current`] and still
//!   holds — so the error stays a lightweight redirect rather than a second copy of the
//!   lineage. The R3 spike (`notes/plan/spikes/R3_SPIKE_REPORT.md` §3, "Explicit agent
//!   protocol state") proved the artifact shape this generalizes: "rejection of continuation
//!   under a different workspace snapshot."
//!
//! # What "superseded" means for a [`Fork`] value, precisely
//!
//! [`Fork`] itself remembers exactly three identities — `origin`, one step of `parent`, and
//! the current head (`lineage.rs`'s "Provenance is three questions, not one") — never the
//! whole chain a lineage may have passed through many advances. [`check_current`] can
//! therefore *prove* superseded status only for an identity equal to `origin` or to `parent`;
//! an identity that was briefly a head two or more advances back, and has since fallen out of
//! a `Fork` value's own memory, is reported [`UnknownSnapshot`] — an honest statement about
//! what *this* `Fork` value can prove, never a false claim that the identity was never part of
//! any lineage. Guessing `Stale` without proof would risk telling a caller to rebase onto
//! nothing recoverable; guessing `Current` (accepting silently) would defeat the whole
//! deliverable. Refusing as `Unknown` is the only answer that never overclaims in either
//! direction. A caller that needs deeper history holds the intermediate `Fork` values
//! themselves — advancing never destroys an earlier value (`lineage.rs`) — or asks the store
//! once sealed publications carry the schema's `parent` chain.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::lineage::{Fork, ForkName};
//! use continuum_workspace::overlay::Overlay;
//! use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
//! use continuum_workspace::staleness::{LineageError, check_current, derive_current};
//! # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! # use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
//! # struct HexIdentity;
//! # impl ContentIdentifier for HexIdentity {
//! #     fn identify(&self, class: ArtifactClass, content: &[u8])
//! #         -> Result<ArtifactHandle, IdentityUnavailable> {
//! #         let mut token = String::with_capacity(content.len() * 2);
//! #         for byte in content {
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
//! #         }
//! #         ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
//! #     }
//! # }
//! let mut content = WorkspaceContent::new();
//! content.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! let base = Snapshot::build(&content, &HexIdentity)?;
//! let old_head = base.identity().clone();
//!
//! let fork = Fork::diverge(ForkName::new("alice")?, &base);
//! assert!(check_current(&fork, &old_head).is_ok());
//!
//! let mut overlay = Overlay::new();
//! overlay.write(WorkspacePath::new("src/lib.rs")?, b"fn main() { todo!() }".to_vec())?;
//! let advanced = fork.advance(&overlay, &HexIdentity)?;
//!
//! // The identity that used to be current is now superseded, and the error names the
//! // lineage's actual current head so the caller can redo its step against it.
//! let error = check_current(&advanced, &old_head).unwrap_err();
//! let LineageError::Stale(stale) = error else {
//!     panic!("expected Stale")
//! };
//! assert_eq!(stale.stale(), &old_head);
//! assert_eq!(stale.current(), advanced.identity());
//!
//! // The guarded derivation refuses the same superseded base outright, before spending a
//! // derivation on it.
//! assert!(derive_current(&overlay, &base, &advanced, &HexIdentity).is_err());
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;

use crate::artifact_path::ArtifactHandle;
use crate::components::WorkspaceDescriptor;
use crate::lineage::{Fork, ForkName};
use crate::overlay::{Overlay, OverlayError, OverlaySnapshot};
use crate::publication::{CapabilityToken, ContentIdentifier, ReferenceStore};
use crate::seal::{SealError, SealedWorkspace};
use crate::snapshot::Snapshot;

/// A snapshot identity that names a real, earlier point in a fork's lineage — its origin or
/// a head it once had — but not its current head.
///
/// Not a dead end: [`StaleSnapshot::current`] names exactly the identity [`Fork::advance`] or
/// [`Fork::advance_to`] need to redo the caller's step against the head that actually exists
/// now. That is deliberately *all* this carries of the fork, rather than a clone of the
/// [`Fork`] value itself: the caller already holds the `Fork` it passed to [`check_current`],
/// so handing back a second copy would only make this error heavier for no recovery benefit
/// — [`ForkName`] is kept for a readable message, not as a second identity (`lineage.rs`: "A
/// fork's name is a label, never an identity"). See the module documentation for how this
/// matches RFC 0026/0027's recovery philosophy at a layer with no wire and no
/// `NextOperation` to carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleSnapshot {
    stale: ArtifactHandle,
    current: ArtifactHandle,
    fork: ForkName,
}

impl StaleSnapshot {
    /// The identity that was superseded.
    #[must_use]
    pub const fn stale(&self) -> &ArtifactHandle {
        &self.stale
    }

    /// The lineage's actual current head — what a caller rebases onto.
    #[must_use]
    pub const fn current(&self) -> &ArtifactHandle {
        &self.current
    }

    /// The label of the fork this staleness was found against.
    #[must_use]
    pub const fn fork(&self) -> &ForkName {
        &self.fork
    }
}

impl fmt::Display for StaleSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` has been superseded in fork `{}`; its current head is `{}` — re-derive or \
             re-advance against that instead",
            self.stale, self.fork, self.current,
        )
    }
}

impl core::error::Error for StaleSnapshot {}

/// A snapshot identity a fork's lineage cannot place — not as its origin, not as its
/// immediately superseded head, and not as its current head.
///
/// Distinct from [`StaleSnapshot`] on purpose (INV-008): this module never asserts
/// "superseded" for an identity it cannot prove is part of the lineage, and never silently
/// treats an unplaceable identity as current either. Both refusals are conservative; neither
/// guesses. See the module documentation, "What 'superseded' means for a `Fork` value,
/// precisely", for exactly what "cannot place" can mean once a fork has advanced more than
/// once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSnapshot {
    identity: ArtifactHandle,
    current: ArtifactHandle,
    fork: ForkName,
}

impl UnknownSnapshot {
    /// The identity that is not recognized.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        &self.identity
    }

    /// The lineage's current head, offered as the nearest known-good point.
    #[must_use]
    pub const fn current(&self) -> &ArtifactHandle {
        &self.current
    }

    /// The label of the fork this identity was checked against.
    #[must_use]
    pub const fn fork(&self) -> &ForkName {
        &self.fork
    }
}

impl fmt::Display for UnknownSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not recognized in fork `{}`'s lineage — not its origin, its most \
             recently replaced head, or its current head `{}`",
            self.identity, self.fork, self.current,
        )
    }
}

impl core::error::Error for UnknownSnapshot {}

/// The typed answer to "is this identity current in this lineage" — see [`check_current`].
///
/// The two variants carry distinct payload types rather than a shared struct with a tag,
/// so a caller that matches one arm gets a value that could not accidentally be the other
/// (INV-008: distinct outcomes stay distinct in the type, not only in a label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageError {
    /// The identity names a real, earlier point in the lineage.
    Stale(StaleSnapshot),
    /// The identity names no point in the lineage this [`Fork`] value can prove.
    Unknown(UnknownSnapshot),
}

impl fmt::Display for LineageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale(error) => error.fmt(f),
            Self::Unknown(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for LineageError {}

impl From<StaleSnapshot> for LineageError {
    fn from(error: StaleSnapshot) -> Self {
        Self::Stale(error)
    }
}

impl From<UnknownSnapshot> for LineageError {
    fn from(error: UnknownSnapshot) -> Self {
        Self::Unknown(error)
    }
}

/// Ask whether `identity` is `fork`'s current head, a real earlier point in its lineage, or
/// neither.
///
/// `Ok(())` when `identity` is the current head — including the degenerate case where an
/// advance produced a head identical in content to what it replaced
/// ([`Fork::advance_to`]'s own doc note: "advancing to a snapshot equal to the current head
/// is legal"). Content equality decides current-ness, not step count, so that case is `Ok`,
/// never `Stale`.
///
/// Determinism: the result is a pure function of `fork`'s three fields (`origin`, `parent`,
/// `head`) and `identity` alone — two `Fork` values reaching the same state through different
/// sequences of `advance`/`advance_to` calls yield the same verdict for the same identity.
///
/// # Errors
///
/// [`LineageError::Stale`] when `identity` equals `fork`'s `origin` or its immediate
/// `parent` and is not equal to its current head; [`LineageError::Unknown`] otherwise — see
/// the module documentation for what "otherwise" can mean once a fork has advanced more than
/// once.
pub fn check_current(fork: &Fork, identity: &ArtifactHandle) -> Result<(), LineageError> {
    if identity == fork.identity() {
        return Ok(());
    }
    if identity == fork.origin() || Some(identity) == fork.parent() {
        return Err(LineageError::Stale(StaleSnapshot {
            stale: identity.clone(),
            current: fork.identity().clone(),
            fork: fork.name().clone(),
        }));
    }
    Err(LineageError::Unknown(UnknownSnapshot {
        identity: identity.clone(),
        current: fork.identity().clone(),
        fork: fork.name().clone(),
    }))
}

/// Why [`derive_current`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedOverlayError {
    /// `base` was not `fork`'s current head. See [`check_current`].
    Lineage(LineageError),
    /// `base` was current, but the derivation itself was refused.
    Overlay(OverlayError),
}

impl fmt::Display for GuardedOverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lineage(error) => error.fmt(f),
            Self::Overlay(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for GuardedOverlayError {}

impl From<LineageError> for GuardedOverlayError {
    fn from(error: LineageError) -> Self {
        Self::Lineage(error)
    }
}

/// Derive an overlay against `fork`'s current head, refusing a `base` [`check_current`] does
/// not accept.
///
/// The unguarded [`Overlay::derive`] takes any `&Snapshot` on the caller's word — right for a
/// caller with its own coordination (an offline diff against a fixed base, for one; the
/// unguarded form stays exactly as it is for that caller). This is for a caller deriving
/// against a *live* fork that other work may have advanced first: it checks `base` before
/// spending a derivation on it, so a buffer built against a superseded base is refused with
/// the current head attached, rather than silently producing a snapshot the lineage has
/// already moved past.
///
/// # Errors
///
/// [`GuardedOverlayError::Lineage`] per [`check_current`] against `base`'s identity;
/// [`GuardedOverlayError::Overlay`] exactly as [`Overlay::derive`] reports it.
pub fn derive_current<I: ContentIdentifier + ?Sized>(
    overlay: &Overlay,
    base: &Snapshot,
    fork: &Fork,
    identifier: &I,
) -> Result<OverlaySnapshot, GuardedOverlayError> {
    check_current(fork, base.identity())?;
    overlay
        .derive(base, identifier)
        .map_err(GuardedOverlayError::Overlay)
}

/// Why [`advance_current`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedAdvanceError {
    /// `expected_head` was not `fork`'s current head. See [`check_current`].
    Lineage(LineageError),
    /// `expected_head` was current, but the overlay derivation was refused.
    Overlay(OverlayError),
}

impl fmt::Display for GuardedAdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lineage(error) => error.fmt(f),
            Self::Overlay(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for GuardedAdvanceError {}

impl From<LineageError> for GuardedAdvanceError {
    fn from(error: LineageError) -> Self {
        Self::Lineage(error)
    }
}

/// Advance `fork` to `head`, refusing when `expected_head` is no longer `fork`'s current
/// head.
///
/// The unguarded [`Fork::advance_to`] is a pure value transformation on `fork` itself and
/// always succeeds — right for a caller that already holds the exact `Fork` value it means
/// to advance, which is most callers, most of the time; the unguarded form stays exactly as
/// it is for that caller. This is the compare-and-set spelling for a caller that computed
/// `head` against a *remembered* identity, `expected_head`, and wants the step refused if
/// another advance landed first — the crate-level shape of "the named snapshot is not
/// current" (RFC 0026's `StaleSnapshot`, `rule errors.common`).
///
/// # Errors
///
/// [`LineageError`] per [`check_current`] against `expected_head`.
pub fn advance_to_current(
    fork: &Fork,
    expected_head: &ArtifactHandle,
    head: Snapshot,
) -> Result<Fork, LineageError> {
    check_current(fork, expected_head)?;
    Ok(fork.advance_to(head))
}

/// [`advance_to_current`]'s overlay-driven spelling: advance `fork` by deriving `overlay`
/// against its current head, refusing when `expected_head` is no longer that head.
///
/// # Errors
///
/// [`GuardedAdvanceError::Lineage`] per [`check_current`] against `expected_head`;
/// [`GuardedAdvanceError::Overlay`] exactly as [`Fork::advance`] reports it.
pub fn advance_current<I: ContentIdentifier + ?Sized>(
    fork: &Fork,
    expected_head: &ArtifactHandle,
    overlay: &Overlay,
    identifier: &I,
) -> Result<Fork, GuardedAdvanceError> {
    check_current(fork, expected_head)?;
    fork.advance(overlay, identifier)
        .map_err(GuardedAdvanceError::Overlay)
}

/// Why [`seal_current`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedSealError {
    /// `descriptor`'s source tree was not `fork`'s current head. See [`check_current`].
    Lineage(LineageError),
    /// The source tree was current, but the seal itself was refused.
    Seal(SealError),
}

impl fmt::Display for GuardedSealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lineage(error) => error.fmt(f),
            Self::Seal(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for GuardedSealError {}

impl From<LineageError> for GuardedSealError {
    fn from(error: LineageError) -> Self {
        Self::Lineage(error)
    }
}

/// Seal `descriptor` only when its source tree is still `fork`'s current head.
///
/// The unguarded [`SealedWorkspace::seal`] publishes whatever descriptor it is given — right
/// for a caller sealing a value with no attached lineage, or one that has already checked
/// staleness through its own coordination (`seal.rs`'s module doc: sealing "does not
/// re-derive anything"); the unguarded form stays exactly as it is for that caller. This is
/// for a caller sealing a descriptor built from a fork's head that may have advanced since
/// the descriptor was built: it refuses to spend a publication on content the lineage has
/// already superseded, matching the wire contract's `workspace.seal`
/// (`schemas/continuumd-native-protocol.idl`), which names an input snapshot and — per `rule
/// errors.common` — MAY refuse it with `StaleSnapshot` when that snapshot is not current.
///
/// # Errors
///
/// [`GuardedSealError::Lineage`] per [`check_current`] against `descriptor.source()`'s
/// identity; [`GuardedSealError::Seal`] exactly as [`SealedWorkspace::seal`] reports it.
pub fn seal_current(
    descriptor: WorkspaceDescriptor,
    fork: &Fork,
    store: &ReferenceStore,
    capability: &CapabilityToken,
) -> Result<SealedWorkspace, GuardedSealError> {
    check_current(fork, descriptor.source().identity())?;
    SealedWorkspace::seal(descriptor, store, capability).map_err(GuardedSealError::Seal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_path::ArtifactClass;
    use crate::lineage::ForkName;
    use crate::publication::{
        ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, IdentityUnavailable,
    };
    use crate::snapshot::{WorkspaceContent, WorkspacePath};

    #[derive(Clone)]
    struct HexIdentity;

    impl ContentIdentifier for HexIdentity {
        fn identify(
            &self,
            class: ArtifactClass,
            content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            let mut token = String::with_capacity(content.len() * 2);
            for byte in content {
                token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
                token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
            }
            ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
        }
    }

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("a test path is valid")
    }

    fn name(text: &str) -> ForkName {
        ForkName::new(text).expect("a test name is valid")
    }

    fn snapshot(bytes: &[u8]) -> Snapshot {
        let mut content = WorkspaceContent::new();
        content
            .insert(path("src/lib.rs"), bytes.to_vec())
            .expect("insert");
        Snapshot::build(&content, &HexIdentity).expect("build")
    }

    fn overlay_to(bytes: &[u8]) -> Overlay {
        let mut overlay = Overlay::new();
        overlay
            .write(path("src/lib.rs"), bytes.to_vec())
            .expect("write");
        overlay
    }

    fn store_and_capability() -> (ReferenceStore, CapabilityToken) {
        let capability = CapabilityToken::mint("publisher").expect("token");
        let store = ReferenceStore::builder(HexIdentity, AuditLog::new())
            .capability(CapabilityDescriptor::new(
                capability.clone(),
                ActorId::new("editor"),
                AuthorityLevel::Propose,
            ))
            .build();
        (store, capability)
    }

    #[test]
    fn the_current_head_passes() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        assert_eq!(check_current(&fork, base.identity()), Ok(()));
    }

    #[test]
    fn a_superseded_head_is_stale_with_the_exact_identities_and_names_the_new_head() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let advanced = fork
            .advance(&overlay_to(b"v2"), &HexIdentity)
            .expect("advance");

        let error = check_current(&advanced, base.identity()).expect_err("stale");
        let LineageError::Stale(stale) = error else {
            panic!("expected Stale, not Unknown")
        };
        assert_eq!(stale.stale(), base.identity());
        assert_eq!(stale.current(), advanced.identity());
        assert_eq!(stale.fork(), advanced.name());
        assert_ne!(stale.current(), stale.stale());

        // The now-current head still passes.
        assert_eq!(check_current(&advanced, advanced.identity()), Ok(()));
    }

    #[test]
    fn an_unrelated_identity_is_unknown_not_stale() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let elsewhere = snapshot(b"never in this lineage");

        let error = check_current(&fork, elsewhere.identity()).expect_err("unknown");
        assert!(matches!(error, LineageError::Unknown(_)));
        let LineageError::Unknown(unknown) = error else {
            unreachable!()
        };
        assert_eq!(unknown.identity(), elsewhere.identity());
        assert_eq!(unknown.current(), fork.identity());

        // Never conflated: an unknown identity's Display does not claim staleness, and the
        // two error shapes cannot be pattern-matched as the same thing.
        assert!(!format!("{unknown}").contains("superseded"));
    }

    #[test]
    fn staleness_is_per_lineage() {
        let base = snapshot(b"v1");
        // Fork A never advances: `base` is still its head.
        let fork_a = Fork::diverge(name("alice"), &base);
        // Fork B advances past `base`.
        let fork_b = Fork::diverge(name("bob"), &base)
            .advance(&overlay_to(b"v2"), &HexIdentity)
            .expect("advance");

        assert_eq!(check_current(&fork_a, base.identity()), Ok(()));
        assert!(matches!(
            check_current(&fork_b, base.identity()),
            Err(LineageError::Stale(_))
        ));
    }

    #[test]
    fn the_origin_is_stale_and_a_doubly_superseded_head_is_honestly_unknown() {
        // origin(base) -> A -> B -> C: three advances, so the intermediate head A falls out
        // of the final Fork value's own memory (it only ever remembers origin, one parent,
        // and the head) while origin and the immediate parent B stay provable.
        let base = snapshot(b"origin");
        let head_a = snapshot(b"a");
        let head_b = snapshot(b"b");
        let head_c = snapshot(b"c");

        let fork = Fork::diverge(name("alice"), &base);
        let fork = fork.advance_to(head_a.clone());
        let fork = fork.advance_to(head_b.clone());
        let fork = fork.advance_to(head_c.clone());

        assert_eq!(check_current(&fork, head_c.identity()), Ok(()));
        assert!(matches!(
            check_current(&fork, head_b.identity()),
            Err(LineageError::Stale(_))
        ));
        assert!(matches!(
            check_current(&fork, base.identity()),
            Err(LineageError::Stale(_))
        ));
        // Honestly Unknown, not a false Stale: this Fork value no longer carries head_a.
        assert!(matches!(
            check_current(&fork, head_a.identity()),
            Err(LineageError::Unknown(_))
        ));
    }

    #[test]
    fn same_lineage_state_yields_the_same_verdict_regardless_of_construction_order() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let overlay = overlay_to(b"v2");

        // One-call spelling.
        let via_advance = fork.advance(&overlay, &HexIdentity).expect("advance");

        // Two-call spelling (`lineage.rs`'s documented equivalent): derive the overlay
        // explicitly, then advance to the resulting snapshot.
        let derived = overlay
            .derive(fork.head(), &HexIdentity)
            .expect("derive")
            .into_snapshot();
        let via_advance_to = fork.advance_to(derived);

        assert_eq!(via_advance, via_advance_to);

        for identity in [base.identity(), via_advance.identity()] {
            assert_eq!(
                check_current(&via_advance, identity),
                check_current(&via_advance_to, identity),
            );
        }
    }

    #[test]
    fn guarded_overlay_derivation_refuses_a_stale_base_and_passes_the_current_one() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let advanced = fork
            .advance(&overlay_to(b"v2"), &HexIdentity)
            .expect("advance");

        // The base that used to be current is now stale relative to `advanced`.
        let refused = derive_current(&overlay_to(b"v3"), &base, &advanced, &HexIdentity);
        assert!(matches!(
            refused,
            Err(GuardedOverlayError::Lineage(LineageError::Stale(_)))
        ));

        // Deriving against the actual current head succeeds.
        let accepted = derive_current(&overlay_to(b"v3"), advanced.head(), &advanced, &HexIdentity)
            .expect("derive against the current head");
        assert_ne!(accepted.snapshot().identity(), advanced.identity());
    }

    #[test]
    fn guarded_advance_refuses_a_stale_expected_head_and_passes_the_current_one() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let advanced = fork.advance_to(snapshot(b"v2"));

        // advance_to_current: expecting the old head is refused.
        assert!(matches!(
            advance_to_current(&advanced, base.identity(), snapshot(b"v3")),
            Err(LineageError::Stale(_))
        ));
        let stepped = advance_to_current(&advanced, advanced.identity(), snapshot(b"v3"))
            .expect("expected head was current");
        assert_eq!(stepped.parent(), Some(advanced.identity()));

        // advance_current: same compare-and-set, overlay-driven.
        assert!(matches!(
            advance_current(&advanced, base.identity(), &overlay_to(b"v4"), &HexIdentity),
            Err(GuardedAdvanceError::Lineage(LineageError::Stale(_)))
        ));
        let stepped = advance_current(
            &advanced,
            advanced.identity(),
            &overlay_to(b"v4"),
            &HexIdentity,
        )
        .expect("expected head was current");
        assert_eq!(stepped.parent(), Some(advanced.identity()));
    }

    #[test]
    fn guarded_seal_refuses_a_stale_source_and_passes_the_current_one() {
        let base = snapshot(b"v1");
        let fork = Fork::diverge(name("alice"), &base);
        let advanced = fork.advance_to(snapshot(b"v2"));
        let (store, capability) = store_and_capability();

        // A descriptor built from the now-stale base is refused before anything publishes.
        let stale_descriptor = WorkspaceDescriptor::builder()
            .build(base.clone(), &HexIdentity)
            .expect("descriptor");
        assert!(matches!(
            seal_current(stale_descriptor, &advanced, &store, &capability),
            Err(GuardedSealError::Lineage(LineageError::Stale(_)))
        ));

        // A descriptor built from the current head publishes normally.
        let current_descriptor = WorkspaceDescriptor::builder()
            .build(advanced.head().clone(), &HexIdentity)
            .expect("descriptor");
        let sealed = seal_current(current_descriptor, &advanced, &store, &capability)
            .expect("descriptor was current");
        assert_eq!(sealed.snapshot_identity(), advanced.identity());
    }
}
