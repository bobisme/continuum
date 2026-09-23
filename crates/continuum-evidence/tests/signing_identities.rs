//! Plan §18.6 signing identities, from the client's side (bn-2ee4c, ADR-0054, TEST-9-07).
//!
//! Every test signs a real artifact's ADR-0013 canonical bytes with a real Ed25519 key and
//! verifies it through `SignatureVerifier`, usually from the signature's wire bytes. Keys
//! come from a fixed-seed `KeyEntropy`, so each run is a function of the source alone
//! (INV-005).

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    AllowedSigners, ArtifactSignature, EntropyUnavailable, KeyEntropy, LocalKeyring, MintError,
    Provenance, RevocationReason, SEED_LEN, SignRefusal, Signature, SignatureVerifier,
    SignedArtifactKind, SignerIdentity, SignerStanding, SigningEvent, SigningRegistry,
    StandingError, UnverifiedReason, VerifiedStanding, WireError, Zeroizing,
};
use continuum_value::identity::ContentIdentity;
use continuum_value::value::{Name, Value};

/// A deterministic entropy capability: seed `n` is 32 copies of byte `start + n`.
struct FixedEntropy {
    next: u8,
    draws: usize,
}

impl FixedEntropy {
    const fn new(start: u8) -> Self {
        Self {
            next: start,
            draws: 0,
        }
    }
}

impl KeyEntropy for FixedEntropy {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        let seed = Zeroizing::new([self.next; SEED_LEN]);
        self.next = self.next.wrapping_add(1);
        self.draws += 1;
        Ok(seed)
    }
}

struct NoEntropy;

impl KeyEntropy for NoEntropy {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Err(EntropyUnavailable)
    }
}

fn actor() -> ActorId {
    ActorId::new("human:solo-dev").expect("valid actor")
}

fn artifact(label: &str) -> ContentIdentity {
    let value = Value::record([
        (Name::new("label").expect("name"), Value::text(label)),
        (
            Name::new("verdict").expect("name"),
            Value::text("validated"),
        ),
    ])
    .expect("record");
    ContentIdentity::of(&value)
}

fn allow_all(signer: &SignerIdentity) -> AllowedSigners {
    AllowedSigners::new().allow(signer.clone(), SignedArtifactKind::ALL)
}

#[test]
fn sign_verify_round_trips_for_every_signed_kind_from_wire_bytes() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(1))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    for kind in SignedArtifactKind::ALL {
        let subject = artifact(kind.as_str());
        let signature = registry
            .sign(&signer, kind, &subject)
            .expect("active signer");
        let wire = signature.encode();
        assert_eq!(ArtifactSignature::decode(&wire), Ok(signature.clone()));
        let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
        let provenance = verifier.verify_encoded(kind, &subject, Some(&wire));
        let verified = provenance.verified().expect("a genuine signature verifies");
        assert_eq!(verified.signer(), signer.identity());
        assert_eq!(verified.kind(), kind);
        assert_eq!(verified.standing(), &VerifiedStanding::Active);
    }
}

#[test]
fn a_tampered_payload_downgrades_to_signature_mismatch() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(2))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    let signature = registry
        .sign(&signer, SignedArtifactKind::Receipt, &artifact("receipt-a"))
        .expect("sign");
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    let provenance = verifier.verify(
        SignedArtifactKind::Receipt,
        &artifact("receipt-b"),
        Some(&signature),
    );
    assert_eq!(
        provenance.unverified().map(|u| u.reason()),
        Some(&UnverifiedReason::SignatureMismatch)
    );

    // Flipping one signature bit, then carrying it on the wire, is the same outcome.
    let mut bytes = *signature.signature().as_bytes();
    bytes[0] ^= 0x01;
    let flipped = ArtifactSignature::from_parts(
        signature.kind(),
        signature.signer().clone(),
        Signature::from_bytes(&bytes).expect("64 bytes"),
    );
    let provenance = verifier.verify_encoded(
        SignedArtifactKind::Receipt,
        &artifact("receipt-a"),
        Some(&flipped.encode()),
    );
    assert_eq!(
        provenance.unverified().map(|u| u.reason()),
        Some(&UnverifiedReason::SignatureMismatch)
    );
    // The untampered signature over the untampered payload is the control.
    assert!(
        verifier
            .verify(
                SignedArtifactKind::Receipt,
                &artifact("receipt-a"),
                Some(&signature)
            )
            .verified()
            .is_some()
    );
}

#[test]
fn a_signature_under_the_wrong_key_does_not_verify() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(3);
    let honest = registry.mint(&actor(), &mut entropy).expect("mint");
    let forger = registry.mint(&actor(), &mut entropy).expect("mint");
    let subject = artifact("bundle");
    let forged = registry
        .sign(&forger, SignedArtifactKind::IntentBundle, &subject)
        .expect("sign");
    // The forger relabels its signature as the honest signer's.
    let relabelled = ArtifactSignature::from_parts(
        forged.kind(),
        honest.identity().clone(),
        *forged.signature(),
    );
    let allowed = allow_all(honest.identity());
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    assert_eq!(
        verifier
            .verify(
                SignedArtifactKind::IntentBundle,
                &subject,
                Some(&relabelled)
            )
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::SignatureMismatch)
    );
    // And the forger's own, un-relabelled signature is authentic but not allowed.
    assert_eq!(
        verifier
            .verify(SignedArtifactKind::IntentBundle, &subject, Some(&forged))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::SignerNotAllowed)
    );
}

#[test]
fn a_revoked_signers_signatures_downgrade_and_it_can_no_longer_sign() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(4))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    let subject = artifact("pack");
    let signature = registry
        .sign(&signer, SignedArtifactKind::DomainPack, &subject)
        .expect("sign");
    let record = registry
        .revoke(signer.identity(), &actor(), RevocationReason::Compromised)
        .expect("revoke");
    assert_eq!(
        registry.standing(signer.identity()),
        Some(&SignerStanding::Revoked {
            reason: RevocationReason::Compromised,
            record
        })
    );
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    assert_eq!(
        verifier
            .verify(SignedArtifactKind::DomainPack, &subject, Some(&signature))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::SignerRevoked {
            reason: RevocationReason::Compromised
        })
    );
    assert!(matches!(
        registry.sign(&signer, SignedArtifactKind::DomainPack, &subject),
        Err(SignRefusal::NotActive(SignerStanding::Revoked { .. }))
    ));
    assert!(matches!(
        registry.revoke(signer.identity(), &actor(), RevocationReason::Compromised),
        Err(StandingError::NotActive(_))
    ));
}

#[test]
fn a_rotated_key_keeps_its_signatures_verifiable_but_cannot_sign_again() {
    let mut registry = SigningRegistry::new();
    let mut keyring = LocalKeyring::new();
    let mut entropy = FixedEntropy::new(5);
    let old = keyring
        .signer_or_mint(&mut registry, &actor(), &mut entropy)
        .expect("mint")
        .identity()
        .clone();
    let subject = artifact("receipt-before-rotation");
    let before = registry
        .sign(
            keyring.signer().expect("held"),
            SignedArtifactKind::Receipt,
            &subject,
        )
        .expect("sign");
    let new = keyring
        .rotate(&mut registry, &actor(), &mut entropy)
        .expect("rotate")
        .identity()
        .clone();
    assert_ne!(old, new);
    assert!(matches!(
        registry.standing(&old),
        Some(SignerStanding::Rotated { successor, .. }) if *successor == new
    ));
    assert_eq!(registry.standing(&new), Some(&SignerStanding::Active));

    let allowed = AllowedSigners::new()
        .allow(old.clone(), [SignedArtifactKind::Receipt])
        .allow(new.clone(), [SignedArtifactKind::Receipt]);
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    let verified = verifier
        .verify(SignedArtifactKind::Receipt, &subject, Some(&before))
        .verified()
        .cloned()
        .expect("plan §4.6: a published receipt stays verifiable");
    assert_eq!(
        verified.standing(),
        &VerifiedStanding::Rotated {
            successor: new.clone()
        }
    );

    let after = registry
        .sign(
            keyring.signer().expect("held"),
            SignedArtifactKind::Receipt,
            &subject,
        )
        .expect("the successor signs");
    assert_eq!(after.signer(), &new);
    assert_eq!(
        verifier
            .verify(SignedArtifactKind::Receipt, &subject, Some(&after))
            .verified()
            .map(|v| v.standing().clone()),
        Some(VerifiedStanding::Active)
    );
    let events: Vec<_> = registry
        .audit_log()
        .iter()
        .map(|record| match record.event() {
            SigningEvent::Minted { .. } => "mint",
            SigningEvent::Rotated { .. } => "rotate",
            SigningEvent::Revoked { .. } => "revoke",
            SigningEvent::Superseded { .. } => "supersede",
        })
        .collect();
    assert_eq!(events, ["mint", "mint", "rotate"]);
}

#[test]
fn a_signer_outside_the_allowed_set_or_its_kinds_is_not_trusted() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(6))
        .expect("mint");
    let subject = artifact("bundle");
    let signature = registry
        .sign(&signer, SignedArtifactKind::IntentBundle, &subject)
        .expect("sign");
    for allowed in [
        AllowedSigners::new(),
        AllowedSigners::new().allow(signer.identity().clone(), [SignedArtifactKind::Receipt]),
    ] {
        let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
        assert_eq!(
            verifier
                .verify(SignedArtifactKind::IntentBundle, &subject, Some(&signature))
                .unverified()
                .map(|u| u.reason()),
            Some(&UnverifiedReason::SignerNotAllowed)
        );
    }
}

#[test]
fn a_kind_relabel_is_refused_before_and_after_the_kind_field_is_rewritten() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(7))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    let subject = artifact("receipt");
    let receipt = registry
        .sign(&signer, SignedArtifactKind::Receipt, &subject)
        .expect("sign");
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    assert_eq!(
        verifier
            .verify(SignedArtifactKind::DomainPack, &subject, Some(&receipt))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::KindMismatch {
            expected: SignedArtifactKind::DomainPack,
            found: SignedArtifactKind::Receipt,
        })
    );
    let rewritten = ArtifactSignature::from_parts(
        SignedArtifactKind::DomainPack,
        receipt.signer().clone(),
        *receipt.signature(),
    );
    assert_eq!(
        verifier
            .verify(SignedArtifactKind::DomainPack, &subject, Some(&rewritten))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::SignatureMismatch)
    );
}

type ReasonCheck = fn(&UnverifiedReason) -> bool;

#[test]
fn an_unverifiable_signature_downgrades_to_typed_unverified_provenance_not_fail_open() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(&actor(), &mut FixedEntropy::new(8))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    let subject = artifact("receipt");
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    let cases: [(Option<&[u8]>, ReasonCheck); 3] = [
        (None, |r| *r == UnverifiedReason::Unsigned),
        (Some(b"not a canonical value"), |r| {
            matches!(r, UnverifiedReason::Malformed(WireError::NotCanonical(_)))
        }),
        (Some(&[]), |r| matches!(r, UnverifiedReason::Malformed(_))),
    ];
    for (encoded, expected) in cases {
        let provenance = verifier.verify_encoded(SignedArtifactKind::Receipt, &subject, encoded);
        let Provenance::Unverified(unverified) = provenance else {
            panic!("{encoded:?} verified: fail-open");
        };
        assert_eq!(unverified.kind(), SignedArtifactKind::Receipt);
        assert!(expected(unverified.reason()), "{:?}", unverified.reason());
    }
}

#[test]
fn the_ci_acceptance_check_fails_closed_on_every_unverified_outcome() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(9);
    let signer = registry.mint(&actor(), &mut entropy).expect("mint");
    let stranger = registry.mint(&actor(), &mut entropy).expect("mint");
    let allowed = AllowedSigners::new().allow(
        signer.identity().clone(),
        [SignedArtifactKind::IntentBundle],
    );
    let subject = artifact("inb");
    let genuine = registry
        .sign(&signer, SignedArtifactKind::IntentBundle, &subject)
        .expect("sign")
        .encode();
    let outsider = registry
        .sign(&stranger, SignedArtifactKind::IntentBundle, &subject)
        .expect("sign")
        .encode();
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());

    assert!(
        verifier
            .verify_for_ci_acceptance(SignedArtifactKind::IntentBundle, &subject, Some(&genuine))
            .is_ok()
    );
    for (encoded, reason) in [
        (None, UnverifiedReason::Unsigned),
        (
            Some(outsider.as_slice()),
            UnverifiedReason::SignerNotAllowed,
        ),
    ] {
        let error = verifier
            .verify_for_ci_acceptance(SignedArtifactKind::IntentBundle, &subject, encoded)
            .expect_err("CI fails closed");
        assert_eq!(error.reason(), &reason);
    }
    let error = verifier
        .verify_for_ci_acceptance(
            SignedArtifactKind::IntentBundle,
            &artifact("other"),
            Some(&genuine),
        )
        .expect_err("CI fails closed on a tampered bundle");
    assert_eq!(error.reason(), &UnverifiedReason::SignatureMismatch);

    registry
        .revoke(signer.identity(), &actor(), RevocationReason::Compromised)
        .expect("revoke");
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    assert!(matches!(
        verifier
            .verify_for_ci_acceptance(SignedArtifactKind::IntentBundle, &subject, Some(&genuine))
            .map_err(|e| e.reason().clone()),
        Err(UnverifiedReason::SignerRevoked { .. })
    ));
}

#[test]
fn signatures_are_deterministic_and_pinned() {
    let sign_once = || {
        let mut registry = SigningRegistry::new();
        let signer = registry
            .mint(&actor(), &mut FixedEntropy::new(42))
            .expect("mint");
        registry
            .sign(&signer, SignedArtifactKind::Receipt, &artifact("pinned"))
            .expect("sign")
    };
    let first = sign_once();
    assert_eq!(first, sign_once());
    assert_eq!(first.encode(), sign_once().encode());
    // Known answer for this module's message construction over a fixed seed and artifact.
    // A change to the envelope, the canonical encoding, or the primitive fails here.
    assert_eq!(first.signature().to_token(), PINNED_SIGNATURE);
    assert_eq!(first.signer().handle().as_str(), PINNED_SIGNER_HANDLE);
}

const PINNED_SIGNATURE: &str = "ed25519:dff4767deaf351d2221bc8396ef2e9fc8c61bc874cbddcd3de601b583f81570de71023c3dace70ea7cd4b6ae892feccb3fc6a97362633d64ee2191a4bd810c02";
const PINNED_SIGNER_HANDLE: &str =
    "signer_76be08009ffd9b92cdcc5d1d4246b91f462039a703c937656897e901f26f4078";

#[test]
fn the_local_key_is_minted_on_first_use_once_and_audited() {
    let mut registry = SigningRegistry::new();
    let mut keyring = LocalKeyring::new();
    let mut entropy = FixedEntropy::new(10);
    assert!(matches!(
        keyring.signer_or_mint(&mut registry, &actor(), &mut NoEntropy),
        Err(MintError::Entropy(EntropyUnavailable))
    ));
    assert!(keyring.signer().is_none());
    assert!(registry.audit_log().is_empty());

    let first = keyring
        .signer_or_mint(&mut registry, &actor(), &mut entropy)
        .expect("mint")
        .identity()
        .clone();
    let second = keyring
        .signer_or_mint(&mut registry, &actor(), &mut entropy)
        .expect("held")
        .identity()
        .clone();
    assert_eq!(first, second);
    assert_eq!(entropy.draws, 1);
    let [record] = registry.audit_log() else {
        panic!("exactly one audit record");
    };
    assert_eq!(record.actor(), &actor());
    assert_eq!(record.event(), &SigningEvent::Minted { signer: first });
    assert_eq!(record.sequence(), 0);

    // Re-minting the same key is refused, not silently merged.
    assert_eq!(
        registry
            .mint(&actor(), &mut FixedEntropy::new(10))
            .map(|s| s.identity().clone()),
        Err(MintError::AlreadyMinted)
    );
}

#[test]
fn a_lost_key_is_revoked_and_superseded_through_linked_audit_records() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(11);
    let lost = registry.mint(&actor(), &mut entropy).expect("mint");
    let lost_id = lost.identity().clone();
    drop(lost);
    let successor = registry
        .supersede_lost(&lost_id, &actor(), &mut entropy)
        .expect("supersede");
    assert!(matches!(
        registry.standing(&lost_id),
        Some(SignerStanding::Revoked {
            reason: RevocationReason::KeyLost,
            ..
        })
    ));
    assert_eq!(
        registry.audit_log().last().map(|r| r.event().clone()),
        Some(SigningEvent::Superseded {
            lost: lost_id,
            successor: successor.identity().clone()
        })
    );
}

#[test]
fn an_allowed_signers_set_round_trips_through_its_canonical_value() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(12);
    let a = registry.mint(&actor(), &mut entropy).expect("mint");
    let b = registry.mint(&actor(), &mut entropy).expect("mint");
    let allowed = AllowedSigners::new()
        .allow(a.identity().clone(), [SignedArtifactKind::Receipt])
        .allow(
            b.identity().clone(),
            [
                SignedArtifactKind::IntentBundle,
                SignedArtifactKind::DomainPack,
            ],
        );
    let value = allowed.to_value();
    let decoded = Value::decode(&value.encode()).expect("canonical");
    assert_eq!(AllowedSigners::from_value(&decoded), Ok(allowed.clone()));
    assert_eq!(
        AllowedSigners::from_value(&decoded)
            .expect("decoded")
            .content_identity(),
        allowed.content_identity()
    );
}

/// Metamorphic relation: **serialization round trip** (docs/19 §3). Verifying a signature
/// record in memory and verifying its canonical wire bytes give the same `Provenance`, for a
/// genuine signature and for each downgrade, so the wire form carries everything the
/// verdict depends on.
#[test]
fn the_verdict_is_invariant_under_the_signature_records_serialization_round_trip() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(13);
    let signer = registry.mint(&actor(), &mut entropy).expect("mint");
    let outsider = registry.mint(&actor(), &mut entropy).expect("mint");
    let allowed = allow_all(signer.identity());
    let subject = artifact("round-trip");
    let genuine = registry
        .sign(&signer, SignedArtifactKind::Receipt, &subject)
        .expect("sign");
    let foreign = registry
        .sign(&outsider, SignedArtifactKind::Receipt, &subject)
        .expect("sign");
    let verifier = SignatureVerifier::new(&allowed, &registry, &registry.head());
    for (record, over) in [
        (&genuine, subject.clone()),
        (&genuine, artifact("tampered")),
        (&foreign, subject.clone()),
    ] {
        let in_memory = verifier.verify(SignedArtifactKind::Receipt, &over, Some(record));
        let round_tripped = ArtifactSignature::decode(&record.encode()).expect("canonical");
        assert_eq!(&round_tripped, record);
        assert_eq!(
            verifier.verify(SignedArtifactKind::Receipt, &over, Some(&round_tripped)),
            in_memory
        );
        assert_eq!(
            verifier.verify_encoded(SignedArtifactKind::Receipt, &over, Some(&record.encode())),
            in_memory
        );
    }
}

/// Metamorphic relation: **set/map insertion order** (docs/19 §3). An allowed-signers set
/// built in either order has one canonical value and one content identity, so the identity an
/// intent bundle pins does not depend on how its author listed the signers.
#[test]
fn an_allowed_signers_set_is_invariant_under_insertion_order() {
    let mut registry = SigningRegistry::new();
    let mut entropy = FixedEntropy::new(14);
    let a = registry.mint(&actor(), &mut entropy).expect("mint");
    let b = registry.mint(&actor(), &mut entropy).expect("mint");
    let forward = AllowedSigners::new()
        .allow(a.identity().clone(), [SignedArtifactKind::Receipt])
        .allow(b.identity().clone(), [SignedArtifactKind::DomainPack])
        .allow(a.identity().clone(), [SignedArtifactKind::IntentBundle]);
    let backward = AllowedSigners::new()
        .allow(a.identity().clone(), [SignedArtifactKind::IntentBundle])
        .allow(b.identity().clone(), [SignedArtifactKind::DomainPack])
        .allow(a.identity().clone(), [SignedArtifactKind::Receipt]);
    assert_eq!(forward, backward);
    assert_eq!(forward.to_value().encode(), backward.to_value().encode());
    assert_eq!(forward.content_identity(), backward.content_identity());
}

/// cr-3e3t1j finding 1: a signer the verifier's registry holds no standing for is never
/// presumed active, even when the signature is authentic and the signer is allowed.
#[test]
fn a_signer_with_unknown_standing_is_unverified_not_active() {
    let mut issuer = SigningRegistry::new();
    let signer = issuer
        .mint(&actor(), &mut FixedEntropy::new(15))
        .expect("mint");
    let subject = artifact("unknown-standing");
    let signature = issuer
        .sign(&signer, SignedArtifactKind::Receipt, &subject)
        .expect("sign")
        .encode();
    // A current registry that knows other signers, but not this one.
    let mut other = SigningRegistry::new();
    other
        .mint(&actor(), &mut FixedEntropy::new(16))
        .expect("mint");
    let allowed = allow_all(signer.identity());
    let verifier = SignatureVerifier::new(&allowed, &other, &other.head());
    assert_eq!(
        verifier
            .verify_encoded(SignedArtifactKind::Receipt, &subject, Some(&signature))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::StandingUnknown)
    );
    assert_eq!(
        verifier
            .verify_for_ci_acceptance(SignedArtifactKind::Receipt, &subject, Some(&signature))
            .map_err(|e| e.reason().clone()),
        Err(UnverifiedReason::StandingUnknown)
    );
}

/// cr-3e3t1j finding 1: a verifier holding a registry copied before a revocation reads it
/// as stale against the authoritative head, so the revoked key does not verify there.
#[test]
fn a_stale_registry_missing_a_revocation_cannot_verify() {
    let mut authority = SigningRegistry::new();
    let signer = authority
        .mint(&actor(), &mut FixedEntropy::new(17))
        .expect("mint");
    let subject = artifact("stale");
    let signature = authority
        .sign(&signer, SignedArtifactKind::DomainPack, &subject)
        .expect("sign")
        .encode();
    let stale = authority.clone();
    authority
        .revoke(signer.identity(), &actor(), RevocationReason::Compromised)
        .expect("revoke");
    assert_eq!(
        stale.standing(signer.identity()),
        Some(&SignerStanding::Active)
    );

    let allowed = allow_all(signer.identity());
    let head = authority.head();
    let verifier = SignatureVerifier::new(&allowed, &stale, &head);
    assert_eq!(
        verifier
            .verify_encoded(SignedArtifactKind::DomainPack, &subject, Some(&signature))
            .unverified()
            .map(|u| u.reason()),
        Some(&UnverifiedReason::StandingStale)
    );
    assert_eq!(
        verifier
            .verify_for_ci_acceptance(SignedArtifactKind::DomainPack, &subject, Some(&signature))
            .map_err(|e| e.reason().clone()),
        Err(UnverifiedReason::StandingStale)
    );
    // Against the authoritative registry the revocation is visible.
    let current = SignatureVerifier::new(&allowed, &authority, &head);
    assert!(matches!(
        current
            .verify_encoded(SignedArtifactKind::DomainPack, &subject, Some(&signature))
            .unverified()
            .map(|u| u.reason()),
        Some(UnverifiedReason::SignerRevoked { .. })
    ));
}

/// cr-3e3t1j finding 1: an empty registry vouches for nobody — current or not.
#[test]
fn an_empty_registry_verifies_no_signature() {
    let mut issuer = SigningRegistry::new();
    let signer = issuer
        .mint(&actor(), &mut FixedEntropy::new(18))
        .expect("mint");
    let subject = artifact("empty");
    let signature = issuer
        .sign(&signer, SignedArtifactKind::IntentBundle, &subject)
        .expect("sign")
        .encode();
    let empty = SigningRegistry::new();
    let allowed = allow_all(signer.identity());
    for (head, reason) in [
        (empty.head(), UnverifiedReason::StandingUnknown),
        (issuer.head(), UnverifiedReason::StandingStale),
    ] {
        let verifier = SignatureVerifier::new(&allowed, &empty, &head);
        let Provenance::Unverified(unverified) =
            verifier.verify_encoded(SignedArtifactKind::IntentBundle, &subject, Some(&signature))
        else {
            panic!("an empty registry verified a signature: fail-open");
        };
        assert_eq!(unverified.reason(), &reason);
        assert!(
            verifier
                .verify_for_ci_acceptance(
                    SignedArtifactKind::IntentBundle,
                    &subject,
                    Some(&signature)
                )
                .is_err()
        );
    }
}
