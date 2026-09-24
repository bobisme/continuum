//! The intent acceptance signature chain (RFC 0037 A1–A4, protocol 3.8, bn-3glnv,
//! cr-2unxyh).
//!
//! > **A1.** An acceptance signature MUST cover, at minimum: the accepted contract's `in_*`,
//! > the base contract's `in_*` (or an explicit genesis marker), the `revise-intent`
//! > capability, the accepting principal, and the timestamp — over the canonical encoding
//! > of those fields.
//! >
//! > **A3.** Chain elements are verified in order, and each element's `signer` MUST be
//! > permitted by the local policy for that element's `scope`.
//! >
//! > — RFC 0037, "Acceptance chain"
//!
//! # The statement
//!
//! Each chain element signs one [`Statement`]: the canonical record
//!
//! ```text
//! { accepted_by, base: <in_* of the predecessor> | "genesis", capability: "revise-intent",
//!   domain: "continuum.intent-acceptance.v1", intent: <in_*>, previous: Bytes, timestamp }
//! ```
//!
//! where `previous` is the previous element's encoded signature record (empty for the
//! first), so the elements are ordered: a reordered, spliced, or truncated chain does not
//! verify. The statement is signed as kind `intent-acceptance` (ADR-0054 D4 puts the kind
//! inside the signed message), and it carries its own domain besides, so no other signature
//! is ever an acceptance.
//!
//! # The record
//!
//! A registry record's `chain` (`schemas/intent-registry-record.schema.json`) holds one
//! [`ChainElement`] per signature: `signer` is the signer's `signer_` handle, `signature`
//! the lowercase hex of its encoded signature record, and `scope` is `revise-intent`. The
//! acceptance block's `signature` is the last element's `signature`: the acceptance
//! signature A1 names is the chain's end.
//!
//! # Verification (the stricter reading)
//!
//! Every element must be signed by a signer this registry knows, whose standing is active
//! now — a rotated or revoked signer does not vouch, because there is no trusted time to
//! prove it signed before it retired (ADR-0054, "Rotation cannot date a signature") — and
//! that local policy allows `intent-acceptance`. The statement is rebuilt from the local
//! record, not from the bundle: the local proposal's `in_*`, the local record's
//! predecessor, and the principal and time the request presents. Every failure is one
//! [`ChainFault`]; the wire answer for all of them is `AcceptanceChainInvalid` (A4).

use continuum_evidence::signing::{
    ArtifactSignature, MAX_SIGNATURE_RECORD_LEN, Provenance, VerifiedStanding,
};
use continuum_value::identity::ContentIdentity;
use continuum_value::value::{Name, Value};

use crate::protocol::scalar::IntentHandle;

/// The domain every acceptance statement carries.
pub const ACCEPTANCE_DOMAIN: &str = "continuum.intent-acceptance.v1";

/// The base marker of a lineage's first contract.
pub const GENESIS: &str = "genesis";

/// The one scope an acceptance chain element has.
pub const SCOPE: &str = "revise-intent";

/// The most elements a chain carries.
pub const MAX_CHAIN: usize = 4;

/// One element of a record's `chain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainElement {
    /// The signer's `signer_` handle.
    pub signer: String,
    /// The lowercase hex of the encoded signature record.
    pub signature: String,
    /// The element's scope: always `revise-intent`.
    pub scope: String,
}

/// What an acceptance signs (A1), before the chain position is added.
#[derive(Debug, Clone, Copy)]
pub struct Statement<'a> {
    /// The accepted contract.
    pub intent: &'a IntentHandle,
    /// Its predecessor, or `None` for a lineage's first contract.
    pub base: Option<&'a IntentHandle>,
    /// The accepting principal.
    pub accepted_by: &'a str,
    /// The acceptance time.
    pub timestamp: &'a str,
}

fn name(text: &str) -> Name {
    Name::new(text).expect("a field name constant is canonical")
}

impl Statement<'_> {
    /// The identity a chain element signs, following `previous` (an encoded signature
    /// record, or empty for the first element).
    #[must_use]
    pub fn identity(&self, previous: &[u8]) -> ContentIdentity {
        let value = Value::record([
            (name("accepted_by"), Value::text(self.accepted_by)),
            (
                name("base"),
                Value::text(self.base.map_or(GENESIS, IntentHandle::as_str)),
            ),
            (name("capability"), Value::text(SCOPE)),
            (name("domain"), Value::text(ACCEPTANCE_DOMAIN)),
            (name("intent"), Value::text(self.intent.as_str())),
            (name("previous"), Value::bytes(previous.to_vec())),
            (name("timestamp"), Value::text(self.timestamp)),
        ])
        .expect("distinct field names");
        ContentIdentity::of(&value)
    }
}

/// Why a chain does not verify. Internal: every one is `AcceptanceChainInvalid` on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainFault {
    /// The record carries no chain.
    Empty,
    /// More than [`MAX_CHAIN`] elements.
    TooLong,
    /// An element's scope is not `revise-intent`.
    Scope,
    /// An element's signature is not a hex-encoded signature record within its bound.
    Malformed,
    /// An element's `signer` is not the signer its signature record names.
    SignerMismatch,
    /// An element's signature does not verify over its statement, under local policy, as
    /// `intent-acceptance`, by a signer this registry knows and has not revoked.
    Unverified,
    /// An element's signer is known but no longer active.
    Retired,
    /// The acceptance block's `signature` is not the chain's last element.
    NotLast,
}

/// Lowercase hex.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Read lowercase hex of at most `max` decoded bytes; the length is checked first.
fn from_hex(text: &str, max: usize) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || text.len() / 2 > max {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| Some(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
}

/// The element a signature record makes.
#[must_use]
pub fn element(signature: &ArtifactSignature) -> ChainElement {
    ChainElement {
        signer: signature.signer().handle().as_str().to_owned(),
        signature: to_hex(&signature.encode()),
        scope: SCOPE.to_owned(),
    }
}

/// Verify `chain`, in order, for `statement`, and that `acceptance_signature` is its last
/// element (A1–A4). `verify` is the one production verifier's check of one element as kind
/// `intent-acceptance` over one statement identity, under local policy and the registry
/// (`signing::verify_acceptance_chain` supplies it).
///
/// # Errors
///
/// The first [`ChainFault`].
pub fn verify_chain_with(
    verify: impl Fn(&ContentIdentity, &ArtifactSignature) -> Provenance,
    statement: &Statement<'_>,
    chain: &[ChainElement],
    acceptance_signature: &str,
) -> Result<(), ChainFault> {
    if chain.is_empty() {
        return Err(ChainFault::Empty);
    }
    if chain.len() > MAX_CHAIN {
        return Err(ChainFault::TooLong);
    }
    let mut previous: Vec<u8> = Vec::new();
    for link in chain {
        if link.scope != SCOPE {
            return Err(ChainFault::Scope);
        }
        let bytes =
            from_hex(&link.signature, MAX_SIGNATURE_RECORD_LEN).ok_or(ChainFault::Malformed)?;
        let signature = ArtifactSignature::decode(&bytes).map_err(|_| ChainFault::Malformed)?;
        if signature.signer().handle().as_str() != link.signer {
            return Err(ChainFault::SignerMismatch);
        }
        match verify(&statement.identity(&previous), &signature) {
            Provenance::Verified(verified) if verified.standing() == &VerifiedStanding::Active => {}
            Provenance::Verified(_) => return Err(ChainFault::Retired),
            Provenance::Unverified(_) => return Err(ChainFault::Unverified),
        }
        previous = bytes;
    }
    if chain.last().map(|last| last.signature.as_str()) != Some(acceptance_signature) {
        return Err(ChainFault::NotLast);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::signing::verify_acceptance_chain as verify_chain;
    use continuum_evidence::actor::ActorId;
    use continuum_evidence::signing::{
        AllowedSigners, SignedArtifactKind as Kind, SigningRegistry,
    };
    use continuum_evidence::signing::{
        EntropyUnavailable, KeyEntropy, LocalKeyring, RevocationReason, SEED_LEN, Zeroizing,
    };

    struct Seed(u8);

    impl KeyEntropy for Seed {
        fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
            Ok(Zeroizing::new([self.0; SEED_LEN]))
        }
    }

    fn actor() -> ActorId {
        ActorId::new("human:steward").expect("actor")
    }

    fn handle(text: &str) -> IntentHandle {
        IntentHandle::new(text).expect("an in_")
    }

    struct Fixture {
        registry: SigningRegistry,
        keys: Vec<LocalKeyring>,
        allowed: AllowedSigners,
    }

    fn fixture(signers: u8) -> Fixture {
        let mut registry = SigningRegistry::new();
        let mut keys = Vec::new();
        let mut allowed = AllowedSigners::new();
        for n in 0..signers {
            let mut keyring = LocalKeyring::default();
            let key = keyring
                .signer_or_mint(&mut registry, &actor(), &mut Seed(30 + n))
                .expect("a key");
            allowed = allowed.allow(key.identity().clone(), [Kind::IntentAcceptance]);
            keys.push(keyring);
        }
        Fixture {
            registry,
            keys,
            allowed,
        }
    }

    fn sign_chain(fixture: &mut Fixture, statement: &Statement<'_>) -> Vec<ChainElement> {
        let mut previous = Vec::new();
        let mut chain = Vec::new();
        for keyring in &mut fixture.keys {
            let key = keyring
                .signer_or_mint(&mut fixture.registry, &actor(), &mut Seed(0))
                .expect("held");
            let signature = fixture
                .registry
                .sign(key, Kind::IntentAcceptance, &statement.identity(&previous))
                .expect("active");
            previous = signature.encode();
            chain.push(element(&signature));
        }
        chain
    }

    fn check(
        fixture: &Fixture,
        statement: &Statement<'_>,
        chain: &[ChainElement],
    ) -> Result<(), ChainFault> {
        let last = chain.last().map_or("", |last| last.signature.as_str());
        verify_chain(
            &fixture.allowed,
            &fixture.registry,
            &fixture.registry.head(),
            statement,
            chain,
            last,
        )
    }

    #[test]
    fn every_covered_field_and_every_element_is_bound() {
        let (p, q) = (handle("in_p"), handle("in_q"));
        let base = handle("in_base");
        let statement = Statement {
            intent: &p,
            base: Some(&base),
            accepted_by: "human:steward",
            timestamp: "2026-09-23T00:00:00.000Z",
        };
        let mut fixture = fixture(2);
        let chain = sign_chain(&mut fixture, &statement);
        assert_eq!(check(&fixture, &statement, &chain), Ok(()));

        // Each covered field, changed: the statement is another one.
        let other_base = handle("in_other");
        for mutated in [
            Statement {
                intent: &q,
                ..statement
            },
            Statement {
                base: Some(&other_base),
                ..statement
            },
            Statement {
                base: None,
                ..statement
            },
            Statement {
                accepted_by: "human:someone-else",
                ..statement
            },
            Statement {
                timestamp: "2026-09-24T00:00:00.000Z",
                ..statement
            },
        ] {
            assert_eq!(
                check(&fixture, &mutated, &chain),
                Err(ChainFault::Unverified)
            );
        }
        // Genesis and a revision are two statements.
        let genesis = Statement {
            base: None,
            ..statement
        };
        let genesis_chain = sign_chain(&mut fixture, &genesis);
        assert_eq!(check(&fixture, &genesis, &genesis_chain), Ok(()));
        assert_eq!(
            check(&fixture, &statement, &genesis_chain),
            Err(ChainFault::Unverified)
        );

        // Reordered, truncated, extended, or emptied chains.
        let reordered = vec![chain[1].clone(), chain[0].clone()];
        assert_eq!(
            check(&fixture, &statement, &reordered),
            Err(ChainFault::Unverified)
        );
        let truncated = &chain[1..];
        assert_eq!(
            check(&fixture, &statement, truncated),
            Err(ChainFault::Unverified)
        );
        assert_eq!(
            verify_chain(
                &fixture.allowed,
                &fixture.registry,
                &fixture.registry.head(),
                &statement,
                &chain[..1],
                &chain[1].signature,
            ),
            Err(ChainFault::NotLast),
            "a prefix is not the acceptance the record names"
        );
        assert_eq!(check(&fixture, &statement, &[]), Err(ChainFault::Empty));
        let long: Vec<ChainElement> =
            std::iter::repeat_n(chain[0].clone(), MAX_CHAIN + 1).collect();
        assert_eq!(check(&fixture, &statement, &long), Err(ChainFault::TooLong));

        // Each element's own fields.
        let mut scoped = chain.clone();
        scoped[0].scope = "read".to_owned();
        assert_eq!(check(&fixture, &statement, &scoped), Err(ChainFault::Scope));
        let mut renamed = chain.clone();
        renamed[0].signer = chain[1].signer.clone();
        assert_eq!(
            check(&fixture, &statement, &renamed),
            Err(ChainFault::SignerMismatch)
        );
        let mut garbled = chain.clone();
        garbled[1].signature = format!("{}zz", &garbled[1].signature[2..]);
        assert_eq!(
            check(&fixture, &statement, &garbled),
            Err(ChainFault::Malformed)
        );
        let mut oversized = chain;
        oversized[0].signature = "00".repeat(MAX_SIGNATURE_RECORD_LEN + 1);
        assert_eq!(
            check(&fixture, &statement, &oversized),
            Err(ChainFault::Malformed)
        );
    }

    #[test]
    fn only_a_known_active_signer_local_policy_allows_vouches() {
        let p = handle("in_p");
        let statement = Statement {
            intent: &p,
            base: None,
            accepted_by: "human:steward",
            timestamp: "2026-09-23T00:00:00.000Z",
        };
        let mut fixture = fixture(1);
        let chain = sign_chain(&mut fixture, &statement);
        // Not allowed for acceptance by local policy.
        let signer = fixture
            .allowed
            .iter()
            .next()
            .map(|(s, _)| s.clone())
            .expect("one");
        let bundle_only = AllowedSigners::new().allow(signer.clone(), [Kind::IntentBundle]);
        assert_eq!(
            verify_chain(
                &bundle_only,
                &fixture.registry,
                &fixture.registry.head(),
                &statement,
                &chain,
                &chain[0].signature,
            ),
            Err(ChainFault::Unverified)
        );
        // Unknown to the registry.
        let empty = SigningRegistry::new();
        assert_eq!(
            verify_chain(
                &fixture.allowed,
                &empty,
                &empty.head(),
                &statement,
                &chain,
                &chain[0].signature
            ),
            Err(ChainFault::Unverified)
        );
        // Revoked.
        let mut revoked = fixture.registry.clone();
        revoked
            .record_observed(
                &actor(),
                continuum_evidence::signing::SigningEvent::Revoked {
                    signer: signer.clone(),
                    reason: RevocationReason::Compromised,
                },
            )
            .expect("revoke");
        assert_eq!(
            verify_chain(
                &fixture.allowed,
                &revoked,
                &revoked.head(),
                &statement,
                &chain,
                &chain[0].signature
            ),
            Err(ChainFault::Unverified)
        );
        // Rotated away: a retired key does not vouch.
        let mut rotated = fixture.registry.clone();
        let key = fixture.keys[0]
            .signer_or_mint(&mut fixture.registry, &actor(), &mut Seed(0))
            .expect("held");
        rotated
            .rotate(key, &actor(), &mut Seed(99))
            .expect("rotate");
        assert_eq!(
            verify_chain(
                &fixture.allowed,
                &rotated,
                &rotated.head(),
                &statement,
                &chain,
                &chain[0].signature
            ),
            Err(ChainFault::Retired)
        );
    }
}
