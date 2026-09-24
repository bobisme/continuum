//! The `signing` family, and the daemon's signing authority (plan §18.6, ADR-0054,
//! protocol 3.8, bn-3glnv).
//!
//! > Receipts, intent bundles, and domain packs are signed. […] identities are minted
//! > through audited daemon operations […]; organizational deployments pin an
//! > allowed-signers set distributed inside the intent bundle; rotation and revocation are
//! > audited operations; a signature that cannot be verified downgrades the artifact to
//! > typed unverified provenance rather than failing open — except the §4.2.1 CI
//! > acceptance check, which fails closed by policy.
//! >
//! > — plan §18.6
//!
//! # What the authority holds
//!
//! [`SigningAuthority`] is part of [`DaemonState`]: one [`SigningRegistry`] (standing and
//! the append-only audit log), at most one held [`LocalSigner`] (the deployment's key; the
//! daemon holds it and agents never do, INV-015), the local [`AllowedSigners`] policy, the
//! held intent bundles, and the [`KeyEntropy`] capability the deployment supplied. Nothing
//! here reads a clock, a file, or the operating system's entropy (INV-005): the entropy is
//! a capability handed in by the builder, and a daemon without one refuses to mint.
//!
//! # The operations, and the rules they keep
//!
//! | Operation | Level | What it does |
//! |---|---|---|
//! | `signing.mint` | `revise-intent`, privileged | mint the held key when none is active; allow it for the named kinds |
//! | `signing.rotate` | `revise-intent`, privileged | retire the held key for a fresh successor, which inherits its kinds |
//! | `signing.revoke` | `revise-intent`, privileged | revoke any known signer; `key-lost` on the held key mints a linked successor |
//! | `signing.registry` | `read` | the audit log, its head digest, and the allowed set |
//! | `signing.verify` | `read` | typed provenance for a signature over caller bytes |
//! | `signing.sign_pack` | `revise-intent`, privileged | sign a domain pack with the held key |
//!
//! Every one needs a grant scoped to no part of the deployment
//! ([`admission::requires_unscoped_grant`](super::admission::requires_unscoped_grant)),
//! and every one is served only on a connection negotiated at 3.8 or later. The registry
//! only grows: nothing un-revokes, un-rotates, or truncates it, and a replayed request is
//! answered from the idempotency ledger rather than run again (`rule idempotency.replay`),
//! so a retried rotation does not rotate twice. No response, fault detail, or `Debug`
//! output carries key material: a signer travels as its [`SignerHandle`] and public key.
//!
//! # Verification never fails open
//!
//! Every check runs through [`SignatureVerifier`], whose outcome is total and typed, against
//! this daemon's registry and the head it refreshes on every change. A bundle's audit log
//! is never replayed verbatim: its records are not signed one by one, so each is read as a
//! fact about its signer's own lineage, and only the facts that cannot widen trust are
//! applied, only when the bundle verifies under them with an active signer, and only after
//! every other check of the import passed ([`SigningAuthority::verify_bundle`],
//! [`SigningAuthority::adopt`]). A bundle never changes the standing of another lineage's
//! signer, nor of any key this daemon minted. `intent.accept` reaches
//! [`SignatureVerifier::verify_for_ci_acceptance`] through
//! [`SigningAuthority::check_acceptance_chain`], which fails closed and also refuses a
//! retired signer.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_evidence::actor::ActorId as SigningActor;
use continuum_evidence::signing::{
    AllowedSigners, ArtifactSignature, KeyEntropy, LinkEvent, LocalSigner, MintError, Provenance,
    RegistryHead, RevocationReason as LibraryReason, SignatureVerifier, SignedArtifactKind as Kind,
    SignerIdentity, SignerLink, SignerStanding, SigningEvent, SigningRegistry, StandingError,
    UnverifiedReason, VerifiedStanding,
};
use continuum_value::identity::ContentIdentity;
use continuum_workspace::publication::ReferenceStore;

use super::Services;
use super::acceptance::{ChainElement, ChainFault, Statement, element, verify_chain_with};
use super::bundle::{MAX_AUDIT_RECORDS, MAX_BUNDLE_LINKS, SignedBundle, signed_bytes_identity};
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::DaemonState;
use crate::protocol::envelope::{StructuralVerdictValue, Verdict};
use crate::protocol::operations::signing::{
    SigningMintRequest, SigningMintResponse, SigningRegistryResponse, SigningRevokeRequest,
    SigningRevokeResponse, SigningRotateRequest, SigningRotateResponse, SigningSignPackRequest,
    SigningSignPackResponse, SigningVerifyRequest, SigningVerifyResponse,
};
use crate::protocol::scalar::{IntentBundleHandle, IntentHandle, ProtocolVersion, SignerHandle};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{
    ErrorCode, RevocationReason, SignatureOutcome, SignedArtifactKind, StructuralOutcome,
};

/// The first protocol version that defines the signing operations, the two bundle
/// operations, and `EvidenceGetResponse.signature` (`@since("3.8")`).
pub const SIGNING_SINCE: ProtocolVersion = ProtocolVersion::new(3, 8);

/// The largest artifact `signing.verify` checks or `signing.sign_pack` signs, in bytes.
/// Checked before the bytes are hashed, copied, or wrapped.
pub const MAX_SIGNED_ARTIFACT_LEN: usize = 1 << 20;

/// The most intent bundles the daemon holds.
pub const MAX_HELD_BUNDLES: usize = 1024;

/// The most bytes of held intent bundles (bodies and signatures) the daemon keeps. Charged
/// by length, before a bundle is held.
pub const MAX_HELD_BUNDLE_BYTES: usize = 64 << 20;

/// Every `(operation, code)` pair this family can answer with.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("signing.mint", ErrorCode::MalformedRequest),
    ("signing.mint", ErrorCode::PolicyGateFailed),
    ("signing.mint", ErrorCode::QuotaExhausted),
    ("signing.mint", ErrorCode::UnsupportedSemanticFeature),
    ("signing.rotate", ErrorCode::MalformedRequest),
    ("signing.rotate", ErrorCode::PolicyGateFailed),
    ("signing.rotate", ErrorCode::QuotaExhausted),
    ("signing.rotate", ErrorCode::UnsupportedSemanticFeature),
    ("signing.revoke", ErrorCode::MalformedRequest),
    ("signing.revoke", ErrorCode::PolicyGateFailed),
    ("signing.revoke", ErrorCode::QuotaExhausted),
    ("signing.revoke", ErrorCode::UnsupportedSemanticFeature),
    ("signing.registry", ErrorCode::MalformedRequest),
    ("signing.verify", ErrorCode::MalformedRequest),
    ("signing.sign_pack", ErrorCode::MalformedRequest),
    ("signing.sign_pack", ErrorCode::PolicyGateFailed),
];

// ---------------------------------------------------------------------------
// The authority.
// ---------------------------------------------------------------------------

/// Audit records kept back for revocations: a mint, a rotation, or an adopted fact other
/// than a revocation is refused once fewer than this many records remain, so a full
/// registry can always still revoke a compromised key.
pub const REVOCATION_RESERVE: usize = 64;

/// Audit records kept back, inside the revocation reserve, for the held key alone: an
/// adopted revocation, or a local revocation of any other signer, is refused once fewer
/// than this many records remain. Every operation that makes an active held key leaves
/// one record for that key's revocation (a replacing mint needs two, a loss recovery four),
/// so an active held key can always still be revoked; a loss recovery with no room left
/// falls back to a plain revocation.
pub const HELD_KEY_RESERVE: usize = 4;

/// The records a compromise of the active held key needs: its revocation, a replacement
/// mint, and that replacement's own revocation. While the held key is active, no daemon
/// operation leaves fewer free — ordinary facts stop at [`REVOCATION_RESERVE`], other
/// revocations at [`HELD_KEY_RESERVE`], both above this — and install refuses a registry
/// that does. A replacement or a loss-recovery successor is minted into this reserve, so
/// it can in turn be revoked, not necessarily replaced: the log is finite.
pub const RECOVERY_RECORDS: usize = 3;

const _: () = assert!(HELD_KEY_RESERVE >= RECOVERY_RECORDS);

/// Which part of the record bound an operation may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reserve {
    /// A mint, a rotation, or an adopted fact other than a revocation.
    Ordinary,
    /// A revocation of a signer other than the held key, local or adopted.
    Revocation,
    /// A revocation or loss recovery of the held key.
    HeldKey,
}

impl Reserve {
    const fn bound(self) -> usize {
        match self {
            Self::Ordinary => MAX_AUDIT_RECORDS - REVOCATION_RESERVE,
            Self::Revocation => MAX_AUDIT_RECORDS - HELD_KEY_RESERVE,
            Self::HeldKey => MAX_AUDIT_RECORDS,
        }
    }
}

/// The deployment's signing authority, as [`DaemonState`] holds it.
pub struct SigningAuthority {
    registry: SigningRegistry,
    /// The head of `registry`, refreshed on every change to it: the authoritative head a
    /// [`SignatureVerifier`] is built against. A registry changed without refreshing it
    /// would read as stale, never as current.
    head: RegistryHead,
    held: Option<LocalSigner>,
    allowed: AllowedSigners,
    index: BTreeMap<SignerHandle, SignerIdentity>,
    /// Signer handles that name more than one key. Never resolved.
    ambiguous: BTreeSet<SignerHandle>,
    entropy: Option<Box<dyn KeyEntropy + Send + Sync>>,
    bundles: BTreeMap<IntentBundleHandle, SignedBundle>,
    held_bytes: usize,
    /// The contracts an import entered, each with the verified bundle that entered it. Such
    /// a proposal is accepted only through a bundle, never by a local acceptance.
    imported: BTreeMap<IntentHandle, IntentBundleHandle>,
    /// Refusals because a content-addressed handle already named byte-different content
    /// (an `in_*` or `inb_*` identity collision). Counted, never logged with the content.
    identity_collisions: u64,
    /// This daemon's own signers: every signer of the installed registry and every key a
    /// local operation minted. A bundle never changes the standing of one of them.
    own: BTreeSet<SignerIdentity>,
    /// For each key this daemon rotated away from and has not revoked, that key's own
    /// compromise revocation, signed at rotation time while the key was active. The key
    /// itself is wiped when it retires; this link is published only if the key is later
    /// revoked as compromised.
    presigned: BTreeMap<SignerIdentity, SignerLink>,
    /// The links this daemon's keys attested: its rotations and compromise revocations.
    /// Never dropped, so a revocation that succeeded always travels.
    own_links: Vec<SignerLink>,
    /// The links adopted from verified bundles, relayed by export. Deduplicated, and capped
    /// so that together with `own_links` they fit one bundle.
    adopted_links: Vec<SignerLink>,
    /// Imports refused because a carried link was not signed by every key it concerns.
    unattested_links: u64,
}

/// What a verified bundle would change: the registry with its facts applied, and the
/// links that applied them. Built by [`SigningAuthority::verify_bundle`] and committed, or
/// dropped, by the caller.
#[derive(Debug)]
pub struct Adoption {
    registry: SigningRegistry,
    head: RegistryHead,
    applied: u32,
    links: Vec<SignerLink>,
}

impl Adoption {
    /// How many facts this adoption records.
    #[must_use]
    pub const fn applied(&self) -> u32 {
        self.applied
    }
}

/// The facts a bundle carries, applied to a candidate registry.
struct Carried {
    registry: SigningRegistry,
    applied: u32,
    links: Vec<SignerLink>,
}

impl Default for SigningAuthority {
    fn default() -> Self {
        let registry = SigningRegistry::new();
        let head = registry.head();
        Self {
            registry,
            head,
            held: None,
            allowed: AllowedSigners::new(),
            index: BTreeMap::new(),
            ambiguous: BTreeSet::new(),
            entropy: None,
            bundles: BTreeMap::new(),
            held_bytes: 0,
            imported: BTreeMap::new(),
            own: BTreeSet::new(),
            presigned: BTreeMap::new(),
            own_links: Vec::new(),
            adopted_links: Vec::new(),
            unattested_links: 0,
            identity_collisions: 0,
        }
    }
}

impl fmt::Debug for SigningAuthority {
    /// Counts and the held signer's public name; never a key, a seed, or a bundle's bytes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningAuthority")
            .field("records", &self.registry.audit_log().len())
            .field(
                "held",
                &self.held.as_ref().map(|key| key.identity().handle()),
            )
            .field("allowed", &self.allowed.len())
            .field("entropy", &self.entropy.is_some())
            .field("bundles", &self.bundles.len())
            .field("imported", &self.imported.len())
            .field("own", &self.own.len())
            .field("presigned", &self.presigned.len())
            .field("own_links", &self.own_links.len())
            .field("adopted_links", &self.adopted_links.len())
            .field("unattested_links", &self.unattested_links)
            .field("identity_collisions", &self.identity_collisions)
            .finish()
    }
}

/// The wire outcome of a library verification.
fn outcome_of(provenance: &Provenance) -> SignatureOutcome {
    match provenance {
        Provenance::Verified(_) => SignatureOutcome::Verified,
        Provenance::Unverified(unverified) => reason_outcome(unverified.reason()),
    }
}

const fn reason_outcome(reason: &UnverifiedReason) -> SignatureOutcome {
    match reason {
        UnverifiedReason::Unsigned => SignatureOutcome::Unsigned,
        UnverifiedReason::Malformed(_) => SignatureOutcome::Malformed,
        UnverifiedReason::KindMismatch { .. } => SignatureOutcome::KindMismatch,
        UnverifiedReason::SignatureMismatch => SignatureOutcome::SignatureMismatch,
        UnverifiedReason::SignerRevoked { .. } => SignatureOutcome::SignerRevoked,
        UnverifiedReason::StandingStale => SignatureOutcome::StandingStale,
        UnverifiedReason::StandingUnknown => SignatureOutcome::StandingUnknown,
        UnverifiedReason::SignerNotAllowed => SignatureOutcome::SignerNotAllowed,
    }
}

/// The library kind a wire kind names.
#[must_use]
pub const fn library_kind(kind: SignedArtifactKind) -> Kind {
    match kind {
        SignedArtifactKind::Receipt => Kind::Receipt,
        SignedArtifactKind::IntentBundle => Kind::IntentBundle,
        SignedArtifactKind::DomainPack => Kind::DomainPack,
        SignedArtifactKind::IntentAcceptance => Kind::IntentAcceptance,
    }
}

/// The wire name of a signer.
///
/// # Errors
///
/// Never in practice: a signer handle is `signer_` and a 64-digit lowercase digest token.
/// A mismatch is refused rather than assumed.
fn wire_handle(identity: &SignerIdentity) -> Result<SignerHandle, Fault> {
    SignerHandle::new(identity.handle().as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "a signer identity has no well-formed wire name",
        )
    })
}

impl SigningAuthority {
    /// The registry: standing and the audit log.
    #[must_use]
    pub const fn registry(&self) -> &SigningRegistry {
        &self.registry
    }

    /// The held signer's identity, if the daemon holds a key.
    #[must_use]
    pub fn held(&self) -> Option<&SignerIdentity> {
        self.held.as_ref().map(LocalSigner::identity)
    }

    /// The local allowed-signers policy.
    #[must_use]
    pub const fn allowed(&self) -> &AllowedSigners {
        &self.allowed
    }

    /// A held intent bundle.
    #[must_use]
    pub fn bundle(&self, handle: &IntentBundleHandle) -> Option<&SignedBundle> {
        self.bundles.get(handle)
    }

    /// How many requests were refused because a content-addressed handle already named
    /// byte-different content.
    #[must_use]
    pub const fn identity_collisions(&self) -> u64 {
        self.identity_collisions
    }

    /// Count one identity collision. The refusal that follows changes nothing else.
    pub(crate) fn note_identity_collision(&mut self) {
        self.identity_collisions = self.identity_collisions.saturating_add(1);
    }

    /// Whether `handle` already names a held bundle whose bytes differ from `bundle`'s: an
    /// `inb_*` identity collision. The same bytes under the same handle are the same bundle.
    pub(crate) fn holds_other_bundle(
        &self,
        handle: &IntentBundleHandle,
        bundle: &SignedBundle,
    ) -> bool {
        self.bundles.get(handle).is_some_and(|held| {
            held.body_bytes() != bundle.body_bytes() || held.signature() != bundle.signature()
        })
    }

    /// The verified bundle an import entered `intent` from, if an import entered it.
    #[must_use]
    pub fn imported_from(&self, intent: &IntentHandle) -> Option<&IntentBundleHandle> {
        self.imported.get(intent)
    }

    /// Install the deployment's registry and key (the builder's `receipt_signer`). The key
    /// is allowed every kind: it is the deployment's own identity (plan §18.6's
    /// solo-developer default). Installing again replaces the registry and the key; a
    /// deployment installs once.
    pub(crate) fn install(&mut self, registry: SigningRegistry, signer: LocalSigner) {
        self.registry = registry;
        self.refresh();
        self.reindex();
        self.own = self
            .registry
            .signers()
            .map(|(identity, _)| identity.clone())
            .collect();
        self.presigned.clear();
        self.own_links.clear();
        self.adopted_links.clear();
        let identity = signer.identity().clone();
        self.own.insert(identity.clone());
        self.allow_one(identity, Kind::ALL);
        self.held = Some(signer);
    }

    /// Add `allowed` to local policy (the builder's `allowed_signers`).
    pub(crate) fn allow(&mut self, allowed: &AllowedSigners) {
        for (signer, kinds) in allowed.iter() {
            self.allow_one(signer.clone(), kinds.iter().copied());
        }
    }

    /// Supply the entropy capability (the builder's `key_entropy`).
    pub(crate) fn set_entropy(&mut self, entropy: Box<dyn KeyEntropy + Send + Sync>) {
        self.entropy = Some(entropy);
    }

    fn allow_one(&mut self, signer: SignerIdentity, kinds: impl IntoIterator<Item = Kind>) {
        self.allowed = core::mem::take(&mut self.allowed).allow(signer, kinds);
    }

    /// Refresh the cached head after the registry changed. One encoding of the log, run on
    /// each change, never per verification.
    fn refresh(&mut self) {
        self.head = self.registry.head();
    }

    /// Rebuild the name index from the registry. One digest per signer, run only when the
    /// registry is replaced (install, adoption), never per request.
    fn reindex(&mut self) {
        self.index.clear();
        let signers: Vec<SignerIdentity> = self
            .registry
            .signers()
            .map(|(identity, _)| identity.clone())
            .collect();
        for identity in &signers {
            self.index_one(identity);
        }
    }

    /// Name `identity` by its handle. A handle that already names a different key is a
    /// digest collision: the handle is then withdrawn, so it resolves to neither key rather
    /// than to whichever was indexed last (ADR-0013: collisions resolve by exact
    /// comparison).
    fn index_one(&mut self, identity: &SignerIdentity) {
        let Ok(handle) = SignerHandle::new(identity.handle().as_str()) else {
            return;
        };
        if self.ambiguous.contains(&handle) {
            return;
        }
        match self.index.get(&handle) {
            Some(held) if held != identity => {
                self.index.remove(&handle);
                self.ambiguous.insert(handle);
                self.identity_collisions = self.identity_collisions.saturating_add(1);
            }
            Some(_) => {}
            None => {
                self.index.insert(handle, identity.clone());
            }
        }
    }

    /// Sign `artifact` as `kind` with the held key, or `None` when the daemon holds no key.
    ///
    /// # Errors
    ///
    /// Inside the `Some`: the held key is not active, or local policy does not allow it to
    /// sign `kind` — a key minted for packs never signs a receipt that would then verify as
    /// `signer-not-allowed`.
    fn sign_as(
        &self,
        kind: Kind,
        artifact: &ContentIdentity,
    ) -> Option<Result<ArtifactSignature, &'static str>> {
        self.held.as_ref().map(|key| {
            if !self.allowed.permits(key.identity(), kind) {
                return Err("the held key is not allowed to sign this kind");
            }
            self.registry
                .sign(key, kind, artifact)
                .map_err(|_| "the held key is not active")
        })
    }

    /// Sign a receipt with the held key, or `None` when the daemon holds no key: the
    /// receipt is then published unsigned, which a verifier reads as `Unsigned`.
    pub(crate) fn sign_receipt(
        &self,
        artifact: &ContentIdentity,
    ) -> Option<Result<ArtifactSignature, &'static str>> {
        self.sign_as(Kind::Receipt, artifact)
    }

    /// Whether `records` more records fit under the part of the bound `reserve` may use.
    fn room(&self, records: usize, reserve: Reserve) -> Result<(), Fault> {
        if self.registry.audit_log().len().saturating_add(records) > reserve.bound() {
            return Err(Fault::new(
                ErrorCode::QuotaExhausted,
                "the signing registry is at its record bound; nothing was recorded",
            )
            .not_retryable());
        }
        Ok(())
    }

    fn resolve(&self, handle: &SignerHandle) -> Result<SignerIdentity, Fault> {
        self.index.get(handle).cloned().ok_or_else(|| {
            Fault::new(
                ErrorCode::PolicyGateFailed,
                "the signing registry holds no standing for the named signer",
            )
        })
    }

    /// The typed provenance of `signature` over `artifact` under local policy and this
    /// registry, against its cached authoritative head.
    fn provenance(
        &self,
        kind: Kind,
        artifact: &ContentIdentity,
        signature: Option<&[u8]>,
    ) -> Provenance {
        SignatureVerifier::new(&self.allowed, &self.registry, &self.head)
            .verify_encoded(kind, artifact, signature)
    }

    /// Imports refused because a carried link was not signed by every key it concerns.
    #[must_use]
    pub const fn unattested_links(&self) -> u64 {
        self.unattested_links
    }

    /// Count one import refused for an unattested link.
    pub(crate) fn note_unattested_link(&mut self) {
        self.unattested_links = self.unattested_links.saturating_add(1);
    }

    /// The links an export carries: every link this daemon's keys attested, then the
    /// adopted ones that still fit one bundle.
    ///
    /// # Errors
    ///
    /// `QuotaExhausted` when this daemon's own links alone exceed [`MAX_BUNDLE_LINKS`]: an
    /// export never silently leaves one out.
    pub(crate) fn export_links(&self) -> Result<Vec<SignerLink>, Fault> {
        if self.own_links.len() > MAX_BUNDLE_LINKS {
            return Err(Fault::new(
                ErrorCode::QuotaExhausted,
                "this daemon's own signer links exceed what a bundle carries",
            )
            .not_retryable());
        }
        let room = MAX_BUNDLE_LINKS - self.own_links.len();
        Ok(self
            .own_links
            .iter()
            .chain(self.adopted_links.iter().take(room))
            .cloned()
            .collect())
    }

    /// Keep a link this daemon's keys attested. Never dropped.
    fn keep_own_link(&mut self, link: SignerLink) {
        if !self.own_links.contains(&link) {
            self.own_links.push(link);
        }
    }

    /// Keep an adopted link for relay: once, and only while under [`MAX_BUNDLE_LINKS`].
    fn keep_adopted_link(&mut self, link: SignerLink) {
        if self.adopted_links.len() < MAX_BUNDLE_LINKS && !self.adopted_links.contains(&link) {
            self.adopted_links.push(link);
        }
    }

    /// Whether `signer` may be introduced to this registry by its own attestation: local
    /// policy pins it, it is unknown here, and it is not one of this daemon's keys.
    fn introducible(&self, candidate: &SigningRegistry, signer: &SignerIdentity) -> bool {
        candidate.standing(signer).is_none()
            && !self.own.contains(signer)
            && self.allowed.kinds(signer).is_some()
    }

    /// The facts an attested rotation `from → to` adds to `candidate`, or `None` when it
    /// does not apply: either key is one of this daemon's own, or is neither active nor
    /// unknown-and-pinned (an unknown pinned key is minted first).
    fn rotation_facts(
        &self,
        candidate: &SigningRegistry,
        from: &SignerIdentity,
        to: &SignerIdentity,
    ) -> Option<Vec<SigningEvent>> {
        if self.own.contains(from) || self.own.contains(to) {
            return None;
        }
        let mut facts = Vec::new();
        for key in [from, to] {
            match candidate.standing(key) {
                Some(SignerStanding::Active) => {}
                None if self.introducible(candidate, key) => {
                    facts.push(SigningEvent::Minted {
                        signer: key.clone(),
                    });
                }
                _ => return None,
            }
        }
        facts.push(SigningEvent::Rotated {
            from: from.clone(),
            to: to.clone(),
        });
        Some(facts)
    }

    /// This registry with the standing facts `bundle` carries applied — or, when the
    /// bundle's own signature is not its claimed signer's, this registry unchanged.
    ///
    /// The checks run in this order, and nothing is applied until every one has passed:
    ///
    /// 1. the bundle's signature authenticates its body as its claimed signer `S`
    ///    ([`ArtifactSignature::authenticates`]), whatever `S`'s standing;
    /// 2. every carried [`SignerLink`] is signed by every key it concerns
    ///    ([`SignerLink::verify`]), and the rotations form disjoint chains
    ///    ([`links_are_attested`]) — the import refuses such a bundle before this runs, and
    ///    here it adopts nothing.
    ///
    /// Then facts are applied to a copy of this registry, and each is a key's own word,
    /// never another signer's:
    ///
    /// - `S` itself, when local policy pins it and it is unknown: minted (its signature
    ///   proves possession);
    /// - an attested rotation `from → to`: recorded when `from` is active here, or unknown
    ///   and pinned, and `to` is active here, or unknown and pinned; an unknown key is
    ///   minted first. Known or unseen, a predecessor is treated alike: the link's own
    ///   signatures decide, never local familiarity. A successor local policy does not pin
    ///   is not introduced, so adoption is bounded by the pinned set;
    /// - an attested revocation of `k` (as compromised): recorded when `k` is known and not
    ///   yet revoked, or unknown and pinned (then minted first);
    /// - a fact about any of this daemon's own keys is skipped: only a local operation
    ///   changes their standing. A loss recovery never arrives, because a lost key cannot
    ///   sign a link.
    ///
    /// The rotations form disjoint chains (checked first); each chain is applied from its
    /// first key forward, chains in the order of their first keys, then the revocations in
    /// the order of the revoked key. So the result is a function of the link set alone, not
    /// of the carried order. Across bundles the first attested transition of a key wins,
    /// and a replayed link is a no-op because its transition is no longer legal. Mints and
    /// rotations stop at [`REVOCATION_RESERVE`], revocations at [`HELD_KEY_RESERVE`]; a
    /// fact group that does not fit is skipped whole. Legality is decided before a group
    /// is recorded, so recording it cannot fail part-way. Each fact is recorded through
    /// `record_observed` with `actor`, the admitted principal of the call. Nothing is
    /// committed here.
    fn with_carried_facts(&self, bundle: &SignedBundle, actor: &SigningActor) -> Carried {
        let unchanged = || Carried {
            registry: self.registry.clone(),
            applied: 0,
            links: Vec::new(),
        };
        let Ok(signature) = ArtifactSignature::decode(bundle.signature()) else {
            return unchanged();
        };
        let body = signed_bytes_identity(bundle.body_bytes());
        if !signature.authenticates(Kind::IntentBundle, &body)
            || !links_are_attested(&bundle.body().links)
        {
            return unchanged();
        }
        let mut candidate = self.registry.clone();
        let mut applied: u32 = 0;
        let mut used = Vec::new();
        // Record a group whose legality was decided against `candidate`. The bound is
        // checked first, arithmetically; a refusal after that would be a bug, and it
        // discards the whole candidate rather than keep part of a group.
        let mut broken = false;
        let mut record = |candidate: &mut SigningRegistry, facts: Vec<SigningEvent>, bound| {
            if candidate.audit_log().len().saturating_add(facts.len()) > bound {
                return false;
            }
            for fact in facts {
                if candidate.record_observed(actor, fact).is_err() {
                    broken = true;
                    return false;
                }
            }
            true
        };
        let signer = signature.signer();
        if self.introducible(&candidate, signer)
            && record(
                &mut candidate,
                vec![SigningEvent::Minted {
                    signer: signer.clone(),
                }],
                Reserve::Ordinary.bound(),
            )
        {
            applied = applied.saturating_add(1);
        }
        // Rotations, as a graph: `links_are_attested` has already refused a key with two
        // successors, a key with two predecessors, and a cycle, so the rotations are
        // disjoint chains. Each chain is applied from its first key forward, and chains in
        // the order of their first keys, so the result is a function of the link set alone.
        let successor: BTreeMap<&SignerIdentity, (&SignerIdentity, &SignerLink)> = bundle
            .body()
            .links
            .iter()
            .filter_map(|link| match link.event() {
                LinkEvent::Rotated { from, to } => Some((from, (to, link))),
                LinkEvent::Revoked { .. } => None,
            })
            .collect();
        let successors: BTreeSet<&SignerIdentity> = successor.values().map(|(to, _)| *to).collect();
        // `links_are_attested` has already refused cycles and shared keys; the walk still
        // marks each key and stops at the edge count, so it follows no edge twice.
        let mut walked: BTreeSet<&SignerIdentity> = BTreeSet::new();
        for first in successor.keys().filter(|key| !successors.contains(*key)) {
            let mut from = *first;
            while let Some((to, link)) = successor.get(from) {
                if !walked.insert(from) || walked.len() > successor.len() {
                    break;
                }
                if let Some(facts) = self.rotation_facts(&candidate, from, to) {
                    let count = u32::try_from(facts.len()).unwrap_or(u32::MAX);
                    if record(&mut candidate, facts, Reserve::Ordinary.bound()) {
                        applied = applied.saturating_add(count);
                        used.push((*link).clone());
                    }
                }
                from = to;
            }
        }
        // Revocations last, in the order of the revoked key.
        let mut revocations: Vec<(&SignerIdentity, &SignerLink)> = bundle
            .body()
            .links
            .iter()
            .filter_map(|link| match link.event() {
                LinkEvent::Revoked { signer } => Some((signer, link)),
                LinkEvent::Rotated { .. } => None,
            })
            .collect();
        revocations.sort_by(|left, right| left.0.cmp(right.0));
        for (revoked, link) in revocations {
            if self.own.contains(revoked) {
                continue;
            }
            let mut facts = Vec::new();
            match candidate.standing(revoked) {
                Some(SignerStanding::Revoked { .. }) => continue,
                Some(_) => {}
                None if self.introducible(&candidate, revoked) => {
                    facts.push(SigningEvent::Minted {
                        signer: revoked.clone(),
                    });
                }
                None => continue,
            }
            facts.push(link.event().to_event());
            let count = u32::try_from(facts.len()).unwrap_or(u32::MAX);
            if record(&mut candidate, facts, Reserve::Revocation.bound()) {
                applied = applied.saturating_add(count);
                used.push(link.clone());
            }
        }
        if broken {
            return unchanged();
        }
        Carried {
            registry: candidate,
            applied,
            links: used,
        }
    }

    /// Verify a bundle's signature under local policy narrowed by the set it pins, against
    /// this registry with the bundle's facts applied. Changes nothing: the caller commits
    /// the returned [`Adoption`] with [`adopt`](Self::adopt) once every other check passed,
    /// so a refusal after this point has changed nothing either.
    ///
    /// The facts are kept only when the signer is active: a retired key's old signature
    /// still verifies, but it does not vouch for new standing facts.
    ///
    /// # Errors
    ///
    /// The typed outcome, when it is not `verified`.
    pub(crate) fn verify_bundle(
        &self,
        bundle: &SignedBundle,
        actor: &SigningActor,
    ) -> Result<Adoption, SignatureOutcome> {
        let carried = self.with_carried_facts(bundle, actor);
        let allowed = self.allowed.intersection(&bundle.body().allowed);
        let head = carried.registry.head();
        let provenance = SignatureVerifier::new(&allowed, &carried.registry, &head).verify_encoded(
            Kind::IntentBundle,
            &signed_bytes_identity(bundle.body_bytes()),
            Some(bundle.signature()),
        );
        let active = match &provenance {
            Provenance::Verified(verified) => verified.standing() == &VerifiedStanding::Active,
            Provenance::Unverified(_) => return Err(outcome_of(&provenance)),
        };
        if !active || carried.applied == 0 {
            return Ok(Adoption {
                registry: self.registry.clone(),
                head: self.head.clone(),
                applied: 0,
                links: Vec::new(),
            });
        }
        Ok(Adoption {
            registry: carried.registry,
            head,
            applied: carried.applied,
            links: carried.links,
        })
    }

    /// Commit an [`Adoption`] [`verify_bundle`](Self::verify_bundle) built. Monotone: its
    /// facts were appended to a copy of this registry, each a legal transition. Returns
    /// how many facts it recorded.
    pub(crate) fn adopt(&mut self, adoption: Adoption) -> u32 {
        if adoption.applied == 0 {
            return 0;
        }
        self.registry = adoption.registry;
        self.head = adoption.head;
        self.reindex();
        for link in adoption.links {
            self.keep_adopted_link(link);
        }
        adoption.applied
    }

    /// Whether the held key may sign `kind`: held, active, and allowed it by local policy.
    pub(crate) fn may_sign(&self, kind: Kind) -> bool {
        self.held.as_ref().is_some_and(|key| {
            self.allowed.permits(key.identity(), kind)
                && self.registry.standing(key.identity()) == Some(&SignerStanding::Active)
        })
    }

    /// Whether any more bundles may be held, before one is built.
    pub(crate) fn has_room_for_more(&self) -> bool {
        self.bundles.len() < MAX_HELD_BUNDLES && self.held_bytes < MAX_HELD_BUNDLE_BYTES
    }

    /// Whether a bundle charging `bytes` may be held: by count, and by length.
    pub(crate) fn has_room_for_bundle(&self, handle: &IntentBundleHandle, bytes: usize) -> bool {
        self.bundles.contains_key(handle)
            || (self.bundles.len() < MAX_HELD_BUNDLES
                && self.held_bytes.saturating_add(bytes) <= MAX_HELD_BUNDLE_BYTES)
    }

    /// Hold a verified bundle by its identity, charging [`SignedBundle::charged_len`].
    /// Idempotent: a held identity is left as it is.
    pub(crate) fn hold(&mut self, handle: IntentBundleHandle, bundle: SignedBundle) -> bool {
        if self.bundles.contains_key(&handle) {
            return false;
        }
        self.held_bytes = self.held_bytes.saturating_add(bundle.charged_len());
        self.bundles.insert(handle, bundle);
        true
    }

    /// Record that a verified import entered `intent`.
    pub(crate) fn record_import(&mut self, intent: IntentHandle, bundle: IntentBundleHandle) {
        self.imported.entry(intent).or_insert(bundle);
    }

    /// Forget the import record of a proposal that left the registry.
    pub(crate) fn forget_import(&mut self, intent: &IntentHandle) {
        self.imported.remove(intent);
    }

    /// The local allowed-signers entries an export pins: the active signers local policy
    /// allows to sign intent bundles. A retired or revoked entry is never pinned, so the
    /// set does not grow with rotations.
    pub(crate) fn export_pins(&self) -> AllowedSigners {
        let entries: Vec<_> = self
            .allowed
            .iter()
            .filter(|(signer, kinds)| {
                kinds.contains(&Kind::IntentBundle)
                    && self.registry.standing(signer) == Some(&SignerStanding::Active)
            })
            .map(|(signer, kinds)| (signer.clone(), kinds.clone()))
            .collect();
        entries
            .into_iter()
            .fold(AllowedSigners::new(), |set, (signer, kinds)| {
                set.allow(signer, kinds)
            })
    }

    /// Sign an intent bundle body with the held key.
    pub(crate) fn sign_bundle(&self, body_bytes: &[u8]) -> Result<ArtifactSignature, Fault> {
        self.sign_as(Kind::IntentBundle, &signed_bytes_identity(body_bytes))
            .unwrap_or(Err("no key"))
            .map_err(|_| no_active_key())
    }

    /// The plan §4.2.1 CI acceptance check over a held bundle, failing closed
    /// (`rule intent.bundles`, RFC 0037 I3, A2–A4).
    ///
    /// Every step refuses with a static reason and nothing else; the caller answers
    /// `AcceptanceChainInvalid` for all of them, so no step is distinguishable on the wire.
    ///
    /// 1. The bundle is held (an absent bundle is `AcceptanceChainInvalid`).
    /// 2. Its signature passes [`SignatureVerifier::verify_for_ci_acceptance`] under local
    ///    policy narrowed by the set it pins, against this registry with the bundle's own
    ///    standing facts applied, so a revocation the bundle carries counts even if it was
    ///    never adopted, and a revocation this daemon knows counts whatever the bundle says.
    /// 3. The signer is active: a rotated signer's old signatures still verify, but a
    ///    retired key does not vouch for a new acceptance.
    /// 4. It exports `proposal`, with an `accepted` record whose acceptance is the one the
    ///    request presents and whose `supersedes` is the local record's (A2).
    ///
    /// # Errors
    ///
    /// The static reason of the first step that fails.
    pub(crate) fn check_acceptance_chain(
        &self,
        handle: &IntentBundleHandle,
        proposal: &IntentHandle,
        claim: &AcceptanceClaim<'_>,
        actor: &SigningActor,
    ) -> Result<Vec<ChainElement>, AcceptanceFault> {
        let bundle = self
            .bundles
            .get(handle)
            .ok_or(AcceptanceFault::BundleAbsent)?;
        let candidate = self.with_carried_facts(bundle, actor).registry;
        let head = candidate.head();
        let allowed = self.allowed.intersection(&bundle.body().allowed);
        let verified = SignatureVerifier::new(&allowed, &candidate, &head)
            .verify_for_ci_acceptance(
                Kind::IntentBundle,
                &signed_bytes_identity(bundle.body_bytes()),
                Some(bundle.signature()),
            )
            .map_err(|_| AcceptanceFault::BundleUnverified)?;
        if verified.standing() != &VerifiedStanding::Active {
            return Err(AcceptanceFault::BundleSignerRetired);
        }
        let entry = bundle
            .body()
            .contract(proposal)
            .ok_or(AcceptanceFault::NotExported)?;
        // The handle alone does not make the bundle's contract the local one: a colliding
        // identity seam, or a digest collision, could name two bodies alike. The signature
        // covers the bundle's bytes, so it vouches for the local contract only when those
        // bytes are exactly the local canonical bytes.
        if entry.contract != claim.contract {
            return Err(AcceptanceFault::IdentityCollision);
        }
        let record = super::intent::BundleRecord::parse(&entry.record, proposal)
            .map_err(|_| AcceptanceFault::RecordMalformed)?;
        if !record.accepts(claim) {
            return Err(AcceptanceFault::RecordMismatch);
        }
        // A1–A4: the acceptance signature chain, in order, over a statement rebuilt from
        // the local record — never from the bundle's — under local policy alone (A3) and
        // this registry with the bundle's attested facts applied.
        let statement = Statement {
            intent: proposal,
            base: claim.supersedes,
            accepted_by: claim.accepted_by,
            timestamp: claim.timestamp,
        };
        let signature = record
            .acceptance_signature()
            .ok_or(AcceptanceFault::RecordMalformed)?;
        verify_acceptance_chain(
            &self.allowed,
            &candidate,
            &head,
            &statement,
            record.chain(),
            signature,
        )
        .map_err(AcceptanceFault::Chain)?;
        Ok(record.chain().to_vec())
    }

    /// Whether the held key is one local policy allows to sign acceptances — the case in
    /// which a local acceptance is signed, and so must present the admitted principal and
    /// the daemon's own time.
    pub(crate) fn signs_acceptances(&self) -> bool {
        self.held
            .as_ref()
            .is_some_and(|key| self.allowed.permits(key.identity(), Kind::IntentAcceptance))
    }

    /// Sign `statement` as the first element of a chain with the held key: `Ok(None)` when
    /// the daemon holds no key local policy allows to sign acceptances.
    ///
    /// # Errors
    ///
    /// `()` when that key is no longer active: the acceptance is then refused, never
    /// recorded unsigned in its place (the same rule a receipt follows).
    pub(crate) fn sign_acceptance(
        &self,
        statement: &Statement<'_>,
    ) -> Result<Option<ChainElement>, ()> {
        if !self.signs_acceptances() {
            return Ok(None);
        }
        match self.sign_as(Kind::IntentAcceptance, &statement.identity(&[])) {
            Some(Ok(signature)) => Ok(Some(element(&signature))),
            Some(Err(_)) => Err(()),
            None => Ok(None),
        }
    }
}

/// Verify an RFC 0037 A1 acceptance chain for `statement` under `allowed` and `registry`
/// against `head`, through the one production verifier (see
/// [`acceptance`](super::acceptance)).
///
/// # Errors
///
/// The first [`ChainFault`].
pub fn verify_acceptance_chain(
    allowed: &AllowedSigners,
    registry: &SigningRegistry,
    head: &RegistryHead,
    statement: &Statement<'_>,
    chain: &[ChainElement],
    acceptance_signature: &str,
) -> Result<(), ChainFault> {
    let verifier = SignatureVerifier::new(allowed, registry, head);
    verify_chain_with(
        |identity, signature| verifier.verify(Kind::IntentAcceptance, identity, Some(signature)),
        statement,
        chain,
        acceptance_signature,
    )
}

/// Why [`SigningAuthority::check_acceptance_chain`] refused. Internal and typed; every one
/// is `AcceptanceChainInvalid` on the wire (RFC 0037 A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceFault {
    /// The named bundle is not held.
    BundleAbsent,
    /// The bundle's own signature does not pass the fail-closed CI check.
    BundleUnverified,
    /// The bundle's signer is retired.
    BundleSignerRetired,
    /// The bundle does not export the proposal.
    NotExported,
    /// The bundle's contract under the proposal's identity is byte-different content.
    IdentityCollision,
    /// The bundle's record for the proposal is malformed.
    RecordMalformed,
    /// The record's status, principal, signature, time, or predecessor is not the one
    /// presented and held locally.
    RecordMismatch,
    /// The acceptance signature chain does not verify.
    Chain(ChainFault),
}

/// What `intent.accept` presents, compared against the bundle's signed record.
#[derive(Debug, Clone, Copy)]
pub struct AcceptanceClaim<'a> {
    /// The local contract's canonical bytes, which the bundle's entry must equal exactly.
    pub contract: &'a [u8],
    /// `accepted_by`.
    pub accepted_by: &'a str,
    /// `signature`.
    pub signature: &'a str,
    /// `timestamp`.
    pub timestamp: &'a str,
    /// The local record's lineage predecessor (A2).
    pub supersedes: Option<&'a IntentHandle>,
}

/// Whether every link is signed by every key it concerns, no key is revoked twice, and the
/// rotations form disjoint chains: no key has two successors or two predecessors, and no
/// chain is a cycle. The
/// check an import runs before anything else, and the one fact adoption repeats.
#[must_use]
pub fn links_are_attested(links: &[SignerLink]) -> bool {
    links.iter().all(SignerLink::verify) && links_form_chains(links.iter().map(SignerLink::event))
}

/// The topology half of [`links_are_attested`], over the events alone: no key revoked
/// twice, every key with at most one successor and at most one predecessor, and no cycle.
/// Linear in the number of events, with no unbounded walk, whatever the input is.
#[must_use]
pub fn links_form_chains<'a>(events: impl IntoIterator<Item = &'a LinkEvent>) -> bool {
    // First the degrees, with no traversal: any defect answers `false` before a single edge
    // is followed.
    let mut successor: BTreeMap<&SignerIdentity, &SignerIdentity> = BTreeMap::new();
    let mut predecessors = BTreeSet::new();
    let mut revoked = BTreeSet::new();
    for event in events {
        let fresh = match event {
            LinkEvent::Rotated { from, to } => {
                successor.insert(from, to).is_none() && predecessors.insert(to)
            }
            LinkEvent::Revoked { signer } => revoked.insert(signer),
        };
        if !fresh {
            return false;
        }
    }
    // Then cycles. With in- and out-degree at most one, the rotations are disjoint paths
    // and cycles; a cycle is exactly a set of edges no walk from a first key reaches. Each
    // walk marks what it visits and is bounded by the edge count, so no edge is followed
    // twice and no walk outlives the input.
    let mut visited: BTreeSet<&SignerIdentity> = BTreeSet::new();
    for first in successor.keys().filter(|key| !predecessors.contains(*key)) {
        let mut from = *first;
        for _ in 0..successor.len() {
            let Some(to) = successor.get(from) else {
                break;
            };
            if !visited.insert(from) {
                return false;
            }
            from = to;
        }
    }
    visited.len() == successor.len()
}

pub(crate) fn no_active_key() -> Fault {
    Fault::new(
        ErrorCode::PolicyGateFailed,
        "this daemon holds no active signing key",
    )
}

/// Refuse an operation the negotiated version does not define.
///
/// # Errors
///
/// `MalformedRequest` on a connection negotiated below [`SIGNING_SINCE`].
pub(crate) fn require_signing_version(services: &Services) -> Result<(), Fault> {
    if services.negotiated().protocol_version() < SIGNING_SINCE {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "this operation is not defined at the negotiated protocol version",
        )
        .not_retryable());
    }
    Ok(())
}

/// The signing-audit actor of the admitted call: who performed, or adopted, a record.
///
/// # Errors
///
/// `MalformedRequest` in the unreachable case that an admitted wire actor is not an audit
/// actor; the two share one pattern.
pub(crate) fn audit_actor(call: &Call<'_>) -> Result<SigningActor, Fault> {
    actor(call)
}

fn actor(call: &Call<'_>) -> Result<SigningActor, Fault> {
    SigningActor::new(call.grant.actor.as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the admitted actor is not a signing-audit actor",
        )
    })
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}

fn head_bytes(authority: &SigningAuthority) -> Vec<u8> {
    authority.head.digest().to_vec()
}

fn entropy_fault(error: MintError) -> Fault {
    match error {
        MintError::Entropy(_) => Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the key-entropy capability could not supply a seed; nothing was recorded",
        ),
        MintError::AlreadyMinted | MintError::WeakKey => Fault::new(
            ErrorCode::PolicyGateFailed,
            "the minted key is not a new signer; nothing was recorded",
        )
        .not_retryable(),
    }
}

fn standing_fault(error: StandingError) -> Fault {
    match error {
        StandingError::Mint(error) => entropy_fault(error),
        StandingError::UnknownSigner | StandingError::NotActive(_) => Fault::new(
            ErrorCode::PolicyGateFailed,
            "the signer's standing does not admit this transition",
        )
        .not_retryable(),
    }
}

// ---------------------------------------------------------------------------
// The family.
// ---------------------------------------------------------------------------

/// The `signing` namespace's six operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct SigningFamily;

impl OperationFamily for SigningFamily {
    fn namespace(&self) -> &'static str {
        "signing"
    }

    fn scope(&self, _arguments: &Arguments) -> ScopeClaim {
        // A signer is not an instance of any class: the operations name nothing a scope
        // list could hold. Admission requires an unscoped grant instead.
        ScopeClaim::default()
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        _store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        require_signing_version(services)?;
        let authority = state.signing_mut();
        match call.arguments {
            Arguments::SigningMint(request) => mint(call, request, authority),
            Arguments::SigningRotate(request) => rotate(call, request, authority),
            Arguments::SigningRevoke(request) => revoke(call, request, authority),
            Arguments::SigningRegistry(_) => registry(authority),
            Arguments::SigningVerify(request) => verify(request, authority),
            Arguments::SigningSignPack(request) => sign_pack(request, authority),
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

fn mint(
    call: &Call<'_>,
    request: &SigningMintRequest,
    authority: &mut SigningAuthority,
) -> Result<Effect, Fault> {
    // Bounded before the set is built: there are four kinds.
    if request.kinds.len() > Kind::ALL.len() {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a mint names each kind at most once",
        )
        .not_retryable());
    }
    let kinds: BTreeSet<Kind> = request.kinds.iter().copied().map(library_kind).collect();
    if kinds.is_empty() || kinds.len() != request.kinds.len() {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a mint names at least one kind the new signer may sign, each at most once",
        )
        .not_retryable());
    }
    if authority
        .held()
        .is_some_and(|held| authority.registry.standing(held) == Some(&SignerStanding::Active))
    {
        return Err(Fault::new(
            ErrorCode::PolicyGateFailed,
            "this daemon already holds an active signing key; rotate it instead",
        )
        .not_retryable());
    }
    // Replacing a revoked held key may use the held key's reserve, but must leave one
    // record for the new key's own revocation: an active held key can always be revoked.
    if authority.held.is_some() {
        authority.room(2, Reserve::HeldKey)?;
    } else {
        authority.room(1, Reserve::Ordinary)?;
    }
    let actor = actor(call)?;
    let entropy = authority.entropy.as_deref_mut().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this deployment supplied no key-entropy capability",
        )
    })?;
    let key = authority
        .registry
        .mint(&actor, entropy)
        .map_err(entropy_fault)?;
    let identity = key.identity().clone();
    authority.refresh();
    authority.index_one(&identity);
    authority.own.insert(identity.clone());
    authority.allow_one(identity.clone(), kinds);
    authority.held = Some(key);
    Ok(Effect::new(
        Payload::SigningMint(SigningMintResponse {
            signer: wire_handle(&identity)?,
            public_key: identity.public_key().to_vec(),
            registry_head: head_bytes(authority),
        }),
        structural(StructuralOutcome::Created),
    ))
}

fn rotate(
    call: &Call<'_>,
    request: &SigningRotateRequest,
    authority: &mut SigningAuthority,
) -> Result<Effect, Fault> {
    let named = authority.resolve(&request.signer)?;
    // Rotation needs possession of the retiring key, and the daemon possesses exactly one.
    if authority.held() != Some(&named) {
        return Err(Fault::new(
            ErrorCode::PolicyGateFailed,
            "only the held signing key can be rotated",
        )
        .not_retryable());
    }
    authority.room(2, Reserve::Ordinary)?;
    let actor = actor(call)?;
    let SigningAuthority {
        registry,
        held,
        entropy,
        ..
    } = authority;
    let entropy = entropy.as_deref_mut().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this deployment supplied no key-entropy capability",
        )
    })?;
    let current = held.as_ref().ok_or_else(no_active_key)?;
    // Both keys attest the rotation, so another registry can adopt it on their word; the
    // retiring key also signs its own compromise revocation now, so it can be wiped at
    // once and still be revoked across deployments later.
    let attested = registry
        .rotate_attested(current, &actor, entropy)
        .map_err(standing_fault)?;
    let identity = attested.successor.identity().clone();
    // The retiring key drops here, and is wiped (`zeroize`).
    drop(held.replace(attested.successor));
    authority
        .presigned
        .insert(named.clone(), attested.revocation);
    authority.keep_own_link(attested.rotation);
    authority.refresh();
    let kinds: Vec<Kind> = authority
        .allowed
        .kinds(&named)
        .map(|kinds| kinds.iter().copied().collect())
        .unwrap_or_default();
    authority.index_one(&identity);
    authority.own.insert(identity.clone());
    authority.allow_one(identity.clone(), kinds);
    Ok(Effect::new(
        Payload::SigningRotate(SigningRotateResponse {
            retired: request.signer.clone(),
            successor: wire_handle(&identity)?,
            public_key: identity.public_key().to_vec(),
            registry_head: head_bytes(authority),
        }),
        structural(StructuralOutcome::Updated),
    ))
}

fn revoke(
    call: &Call<'_>,
    request: &SigningRevokeRequest,
    authority: &mut SigningAuthority,
) -> Result<Effect, Fault> {
    let named = authority.resolve(&request.signer)?;
    let actor = actor(call)?;
    // A loss recovery that cannot fit, with room for its successor's revocation, falls back
    // to a plain revocation: the lost key is still retired, and no key is made that could
    // not in turn be revoked.
    let lost_and_held = request.reason == RevocationReason::KeyLost
        && authority.held() == Some(&named)
        && authority.room(4, Reserve::HeldKey).is_ok();
    let successor = if lost_and_held {
        // Loss recovery (docs/09 §10): revoke, mint a successor, link the two. It may use the
        // held key's reserve, and leaves one record for the successor's own revocation.
        authority.room(4, Reserve::HeldKey)?;
        let SigningAuthority {
            registry, entropy, ..
        } = authority;
        let entropy = entropy.as_deref_mut().ok_or_else(|| {
            Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "this deployment supplied no key-entropy capability",
            )
        })?;
        // A lost key cannot sign, so no link is made: a peer learns of the loss only from
        // its own operator.
        let successor = registry
            .supersede_lost(&named, &actor, entropy)
            .map_err(standing_fault)?;
        let identity = successor.identity().clone();
        authority.held = Some(successor);
        let kinds: Vec<Kind> = authority
            .allowed
            .kinds(&named)
            .map(|kinds| kinds.iter().copied().collect())
            .unwrap_or_default();
        authority.index_one(&identity);
        authority.own.insert(identity.clone());
        authority.allow_one(identity.clone(), kinds);
        Nullable::Value(wire_handle(&identity)?)
    } else {
        // An active held key always has a record left for its revocation: every operation
        // that makes one leaves it (`signing.mint` replacing, loss recovery), and nothing
        // else may use the held key's reserve.
        let held = authority.held() == Some(&named);
        authority.room(
            1,
            if held {
                Reserve::HeldKey
            } else {
                Reserve::Revocation
            },
        )?;
        let reason = match request.reason {
            RevocationReason::Compromised => LibraryReason::Compromised,
            RevocationReason::KeyLost => LibraryReason::KeyLost,
        };
        // A compromise of a key this daemon holds is attested by that key before it is
        // revoked; one of a key it retired was attested when it retired. Either travels in
        // the next bundle.
        let link = if reason == LibraryReason::Compromised && held {
            authority
                .held
                .as_ref()
                .and_then(|key| authority.registry.attest_revocation(key).ok())
        } else {
            None
        };
        authority
            .registry
            .revoke(&named, &actor, reason)
            .map_err(standing_fault)?;
        let presigned = authority.presigned.remove(&named);
        if reason == LibraryReason::Compromised {
            if let Some(link) = link.or(presigned) {
                authority.keep_own_link(link);
            }
        }
        // A revoked held key stays held, so every later signing refuses rather than
        // publishing unsigned; a later `signing.mint` replaces it.
        Nullable::Null
    };
    authority.refresh();
    Ok(Effect::new(
        Payload::SigningRevoke(SigningRevokeResponse {
            signer: request.signer.clone(),
            successor,
            registry_head: head_bytes(authority),
        }),
        structural(StructuralOutcome::Updated),
    ))
}

fn registry(authority: &SigningAuthority) -> Result<Effect, Fault> {
    let log = authority
        .registry
        .audit_log()
        .iter()
        .map(|record| record.to_value().encode())
        .collect();
    let allowed = authority
        .allowed
        .iter()
        .map(|(signer, kinds)| AllowedSigners::entry_value(signer, kinds).encode())
        .collect();
    Ok(Effect::new(
        Payload::SigningRegistry(SigningRegistryResponse {
            signers: authority.index.keys().cloned().collect(),
            log,
            registry_head: head_bytes(authority),
            allowed,
        }),
        Nullable::Null,
    ))
}

fn verify(request: &SigningVerifyRequest, authority: &SigningAuthority) -> Result<Effect, Fault> {
    if request.artifact.len() > MAX_SIGNED_ARTIFACT_LEN {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "the artifact exceeds the signed-artifact size bound",
        )
        .not_retryable());
    }
    let provenance = authority.provenance(
        library_kind(request.kind),
        &signed_bytes_identity(&request.artifact),
        request.signature.value().map(Vec::as_slice),
    );
    let (signer, successor) = match &provenance {
        Provenance::Verified(verified) => (
            Optional::Present(wire_handle(verified.signer())?),
            match verified.standing() {
                VerifiedStanding::Active => Optional::Absent,
                VerifiedStanding::Rotated { successor } => {
                    Optional::Present(wire_handle(successor)?)
                }
            },
        ),
        Provenance::Unverified(_) => (Optional::Absent, Optional::Absent),
    };
    Ok(Effect::new(
        Payload::SigningVerify(SigningVerifyResponse {
            outcome: outcome_of(&provenance),
            signer,
            successor,
        }),
        Nullable::Null,
    ))
}

fn sign_pack(
    request: &SigningSignPackRequest,
    authority: &SigningAuthority,
) -> Result<Effect, Fault> {
    if request.pack.len() > MAX_SIGNED_ARTIFACT_LEN {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "the pack exceeds the signed-artifact size bound",
        )
        .not_retryable());
    }
    let signature = authority
        .sign_as(Kind::DomainPack, &signed_bytes_identity(&request.pack))
        .unwrap_or(Err("no key"))
        .map_err(|_| no_active_key().not_retryable())?;
    Ok(Effect::new(
        Payload::SigningSignPack(SigningSignPackResponse {
            signature: signature.encode(),
            signer: wire_handle(signature.signer())?,
        }),
        structural(StructuralOutcome::Created),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::bundle::{BundleBody, decode_signed, encode_signed};
    use continuum_evidence::signing::{EntropyUnavailable, LocalKeyring, SEED_LEN, Zeroizing};

    struct Seed(u8);

    impl KeyEntropy for Seed {
        fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
            Ok(Zeroizing::new([self.0; SEED_LEN]))
        }
    }

    fn actor() -> SigningActor {
        SigningActor::new("human:peer").expect("actor")
    }

    fn signed(registry: &SigningRegistry, key: &LocalSigner, body: &BundleBody) -> SignedBundle {
        let bytes = body.encode();
        let signature = registry
            .sign(key, Kind::IntentBundle, &signed_bytes_identity(&bytes))
            .expect("sign");
        decode_signed(&encode_signed(&bytes, &signature.encode())).expect("a bundle")
    }

    /// Exact entropy: the seed given.
    struct Exact([u8; SEED_LEN]);

    impl KeyEntropy for Exact {
        fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
            Ok(Zeroizing::new(self.0))
        }
    }

    /// The topology check at the largest link count a bundle carries: a valid chain is
    /// accepted, and the same chain closed into a cycle, with an edge replayed, or entered
    /// by a tail half-way along, is refused — each in one bounded pass. (Signatures are
    /// checked by `SignerLink::verify` before this, per link; the wire tests cover them.)
    #[test]
    fn the_topology_check_is_bounded_at_the_link_bound() {
        use crate::daemon::bundle::MAX_BUNDLE_LINKS;
        let mut registry = SigningRegistry::new();
        let keys: Vec<SignerIdentity> = (0..=MAX_BUNDLE_LINKS)
            .map(|n| {
                let mut seed = [0u8; SEED_LEN];
                seed[..4].copy_from_slice(&u32::try_from(n).expect("small").to_le_bytes());
                seed[31] = 0x77;
                LocalKeyring::default()
                    .signer_or_mint(&mut registry, &actor(), &mut Exact(seed))
                    .expect("a key")
                    .identity()
                    .clone()
            })
            .collect();
        let rotate = |from: usize, to: usize| LinkEvent::Rotated {
            from: keys[from].clone(),
            to: keys[to].clone(),
        };
        let chain: Vec<LinkEvent> = (0..MAX_BUNDLE_LINKS).map(|at| rotate(at, at + 1)).collect();
        assert!(links_form_chains(&chain), "a chain of the bound");
        let mut reversed = chain.clone();
        reversed.reverse();
        assert!(links_form_chains(&reversed), "in any order");
        let mut cycle = chain.clone();
        cycle.push(rotate(MAX_BUNDLE_LINKS, 0));
        assert!(!links_form_chains(&cycle), "closed into a cycle");
        let mut replayed = chain.clone();
        replayed.push(chain[MAX_BUNDLE_LINKS / 2].clone());
        assert!(!links_form_chains(&replayed), "an edge replayed");
        let mut entered = chain;
        entered.push(rotate(MAX_BUNDLE_LINKS, MAX_BUNDLE_LINKS / 2));
        assert!(!links_form_chains(&entered), "a tail into the chain");
        let revocation = LinkEvent::Revoked {
            signer: keys[0].clone(),
        };
        assert!(
            !links_form_chains(&[revocation.clone(), revocation]),
            "a revocation replayed"
        );
    }

    /// The authentication gate on its own: a body whose links would apply, under a
    /// signature over another body, adopts nothing — before any verifier runs.
    #[test]
    fn facts_are_computed_only_for_a_bundle_that_authenticates() {
        // Keys through the keyring, the deployment's own first-use path.
        let mut peer = SigningRegistry::new();
        let (mut first, mut second) = (LocalKeyring::default(), LocalKeyring::default());
        let a = first
            .signer_or_mint(&mut peer, &actor(), &mut Seed(40))
            .expect("a key");
        let b = second
            .signer_or_mint(&mut peer, &actor(), &mut Seed(41))
            .expect("a key");
        let link = peer.attest_rotation(a, b).expect("active");
        let mut authority = SigningAuthority::default();
        authority.allow(
            &AllowedSigners::new()
                .allow(a.identity().clone(), [Kind::IntentBundle])
                .allow(b.identity().clone(), [Kind::IntentBundle]),
        );
        let body = BundleBody {
            allowed: AllowedSigners::new(),
            contracts: Vec::new(),
            links: vec![link],
        };
        let good = signed(&peer, b, &body);
        let control = authority.with_carried_facts(&good, &actor());
        assert_eq!(
            control.applied, 3,
            "B, A, and the rotation: the gate is not vacuous"
        );
        let other = signed(
            &peer,
            b,
            &BundleBody {
                allowed: AllowedSigners::new(),
                contracts: Vec::new(),
                links: Vec::new(),
            },
        );
        let spliced =
            decode_signed(&encode_signed(good.body_bytes(), other.signature())).expect("a bundle");
        let carried = authority.with_carried_facts(&spliced, &actor());
        assert_eq!(carried.applied, 0);
        assert!(carried.registry.audit_log().is_empty());
    }
}
