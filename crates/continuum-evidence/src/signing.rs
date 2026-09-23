//! Signing identities (plan §18.6, docs/09 §10, ADR-0054).
//!
//! > Receipts, intent bundles, and domain packs are signed. […] identities are minted
//! > through audited daemon operations (the solo-developer default is a local keypair
//! > minted on first use and recorded in the audit log); organizational deployments pin an
//! > allowed-signers set distributed inside the intent bundle; rotation and revocation are
//! > audited operations; a signature that cannot be verified downgrades the artifact to
//! > typed unverified provenance rather than failing open — except the §4.2.1 CI
//! > acceptance check, which fails closed by policy.
//! >
//! > — plan §18.6
//!
//! # What each type owns
//!
//! | Type | Owns |
//! |---|---|
//! | [`SignedArtifactKind`] | the three signed artifact classes, and the domain separation between them |
//! | [`SignerIdentity`] | a signer as a content-addressed value: the canonical record of scheme and public key |
//! | [`Signature`], [`ArtifactSignature`] | the signature bytes, and the canonical wire record that carries them |
//! | [`KeyEntropy`] | the only way entropy reaches key generation (INV-005, ADR-0003) |
//! | [`LocalSigner`], [`LocalKeyring`] | the secret half, and the solo-developer "minted on first use" default |
//! | [`SigningRegistry`] | signer standing, and the append-only audit log of mint, rotation, revocation, and supersession |
//! | [`AllowedSigners`] | the trust root an intent bundle pins: which signer may sign which kind |
//! | [`SignatureVerifier`] | the two verification policies: downgrade ([`Provenance`]) and CI fail-closed ([`AcceptanceChainInvalid`]) |
//!
//! # The signed message
//!
//! A signature never covers bare artifact bytes. It covers the canonical encoding
//! ([`Value::encode`], ADR-0013) of the record
//!
//! ```text
//! { artifact: Bytes(<the artifact's canonical bytes>),
//!   domain:   Text("continuum.signature.v1"),
//!   kind:     Text("receipt" | "intent-bundle" | "domain-pack"),
//!   signer:   <the signer identity's canonical record> }
//! ```
//!
//! so a receipt signature cannot be replayed as a domain-pack signature (the kind is
//! signed), a signature cannot be re-attributed to a second key (the signer is signed), and
//! a later format revision cannot be confused with this one (the domain is signed). The
//! artifact half is its [`ContentIdentity`] — the exact canonical bytes, never a digest of
//! them — so the signature binds what ADR-0013 says the artifact *is*.
//!
//! # Verification never fails open
//!
//! [`SignatureVerifier::verify`] is total and returns a [`Provenance`]: either
//! [`Provenance::Verified`] with the signer and its standing, or
//! [`Provenance::Unverified`] with a typed [`UnverifiedReason`] (INV-008). There is no
//! boolean, and no path from a malformed, tampered, wrong-key, revoked, or not-allowed
//! signature to `Verified`. Nor from an unknown one: `Verified` needs the signer's standing
//! in a registry whose audit-log [`RegistryHead`] equals the authoritative head the
//! verifier was built with. A registry that lacks the signer is
//! [`UnverifiedReason::StandingUnknown`]; one whose head differs, and so may lack a
//! revocation, is [`UnverifiedReason::StandingStale`] (cr-3e3t1j). [`SignatureVerifier::verify_for_ci_acceptance`] is the plan
//! §4.2.1 exception: the same checks, but every unverified outcome is an
//! [`AcceptanceChainInvalid`] error, so the caller cannot proceed on it.
//!
//! # Rotation is not revocation
//!
//! A published receipt stays verifiable under its pinned epoch indefinitely (plan §4.6), so
//! rotating a key must not invalidate what the key already signed. A rotated signer's
//! signature still verifies, with [`SignerStanding::Rotated`] naming its successor; the
//! registry refuses to *sign* with it again. Revocation is the compromise answer: a revoked
//! signer's signatures downgrade to [`UnverifiedReason::SignerRevoked`]. A lost key is
//! revoked as [`RevocationReason::KeyLost`] and superseded by a fresh identity through an
//! audit-linked supersession record (docs/09 §10, "Loss recovery").
//!
//! # What is not here
//!
//! The production signing path is outside this crate (bn-1hape): the operating-system
//! entropy capability and the on-disk keystore are in `continuum-security`, a boundary
//! crate, and `continuumd`'s `evidence.link` signs each receipt it publishes. No production
//! path verifies yet: the daemon operations, the wire spelling of signatures,
//! allowed-signers sets and the authoritative registry head, and `intent.accept`'s use of
//! [`SignatureVerifier::verify_for_ci_acceptance`] are bn-3glnv (ADR-0054, "Follow-ups").
//! The protocol is frozen at 3.6.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::identity::{Blake3Hasher, ContentIdentity, HashIdentity, NonCertifiedLabel};
use continuum_value::value::{DecodeError, Name, Value};
use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
/// Re-exported so a [`KeyEntropy`] implementation needs no direct `zeroize` edge.
pub use zeroize::Zeroizing;

use crate::actor::ActorId;

/// The domain-separation string every signed message carries.
pub const SIGNATURE_DOMAIN: &str = "continuum.signature.v1";

/// Length of an Ed25519 public key.
pub const PUBLIC_KEY_LEN: usize = 32;

/// Length of an Ed25519 signature.
pub const SIGNATURE_LEN: usize = 64;

/// Length of the secret seed a key is derived from (RFC 8032 §5.1.5).
pub const SEED_LEN: usize = 32;

/// The prefix of a signer handle.
pub const SIGNER_HANDLE_PREFIX: &str = "signer_";

/// The ADR-0013 non-certified label a signer handle is minted under.
pub const SIGNER_HANDLE_LABEL: &str = "signer-identity-handle";

/// The prefix of a signature token.
pub const SIGNATURE_TOKEN_PREFIX: &str = "ed25519:";

/// The largest encoded [`ArtifactSignature`] [`ArtifactSignature::decode`] will parse.
///
/// A well-formed record is a fixed shape — three fields, a 64-byte signature, a 32-byte
/// key, and short tokens — and encodes to well under this bound
/// (`a_signature_record_fits_the_decode_bound` pins the margin). Anything longer is refused
/// before the canonical decoder runs, so untrusted bytes cannot make the decoder allocate
/// or recurse in proportion to their length.
pub const MAX_SIGNATURE_RECORD_LEN: usize = 512;

// ---------------------------------------------------------------------------
// Vocabulary.
// ---------------------------------------------------------------------------

/// The signature scheme. Closed at one member (ADR-0054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignatureScheme {
    /// Ed25519 (RFC 8032, pure, strict verification).
    Ed25519,
}

impl SignatureScheme {
    /// The stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }
}

/// The three artifact kinds plan §18.6 says are signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignedArtifactKind {
    /// A receipt (plan §4.4 `receipt_*`, RFC 0032 promotion receipts).
    Receipt,
    /// A signed intent bundle (plan §4.2.1 `inb_*`).
    IntentBundle,
    /// A domain pack (plan §7, docs/09 T07 "pack signature/provenance").
    DomainPack,
}

impl SignedArtifactKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 3] = [Self::Receipt, Self::IntentBundle, Self::DomainPack];

    /// The stable token, signed into every message.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Receipt => "receipt",
            Self::IntentBundle => "intent-bundle",
            Self::DomainPack => "domain-pack",
        }
    }

    /// Parse a kind token.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == token)
    }
}

impl fmt::Display for SignedArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a wire record could not be read as a signing value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The bytes are not a canonical value encoding.
    NotCanonical(DecodeError),
    /// The value does not have the record shape this type encodes to.
    Shape(&'static str),
    /// The scheme token is not one this build verifies.
    UnknownScheme,
    /// The kind token is not one of [`SignedArtifactKind::ALL`].
    UnknownKind,
    /// A public key is the wrong length, not a curve point, or a weak (small-order) point.
    InvalidPublicKey,
    /// A signature is the wrong length or not lowercase hexadecimal.
    InvalidSignature,
    /// An encoded record is longer than [`MAX_SIGNATURE_RECORD_LEN`]; it was not parsed.
    TooLarge,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCanonical(error) => write!(f, "not a canonical value encoding: {error:?}"),
            Self::Shape(what) => write!(f, "unexpected record shape: {what}"),
            Self::UnknownScheme => f.write_str("unknown signature scheme"),
            Self::UnknownKind => f.write_str("unknown signed artifact kind"),
            Self::InvalidPublicKey => f.write_str("invalid or weak Ed25519 public key"),
            Self::InvalidSignature => f.write_str("malformed Ed25519 signature"),
            Self::TooLarge => f.write_str("encoded signature record exceeds its size bound"),
        }
    }
}

impl core::error::Error for WireError {}

fn name(text: &str) -> Name {
    Name::new(text).expect("a field name constant is printable ASCII")
}

fn record(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::record(fields.into_iter().map(|(key, value)| (name(key), value)))
        .expect("field names are distinct constants")
}

fn field<'a>(value: &'a Value, key: &'static str) -> Result<&'a Value, WireError> {
    match value {
        Value::Record(fields) => fields.get(&name(key)).ok_or(WireError::Shape(key)),
        _ => Err(WireError::Shape("record")),
    }
}

fn text_field<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, WireError> {
    match field(value, key)? {
        Value::Text(text) => Ok(text),
        _ => Err(WireError::Shape(key)),
    }
}

fn bytes_field<'a>(value: &'a Value, key: &'static str) -> Result<&'a [u8], WireError> {
    match field(value, key)? {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(WireError::Shape(key)),
    }
}

fn exact_fields(value: &Value, keys: &[&'static str]) -> Result<(), WireError> {
    match value {
        Value::Record(fields) if fields.len() == keys.len() => Ok(()),
        Value::Record(_) => Err(WireError::Shape("unexpected field count")),
        _ => Err(WireError::Shape("record")),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Decode exactly `2 * N` lowercase hexadecimal digits into `[u8; N]`. The length is
/// checked before any byte is decoded, and nothing is allocated, so untrusted text of any
/// length costs at most one comparison to refuse.
fn from_hex_exact<const N: usize>(text: &str) -> Option<[u8; N]> {
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if bytes.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (slot, pair) in out.iter_mut().zip(bytes.chunks_exact(2)) {
        *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Signer identity.
// ---------------------------------------------------------------------------

/// A signer, as a content-addressed value.
///
/// The exact identity is the canonical record `{ public_key: Bytes(32), scheme: "ed25519" }`
/// (ADR-0013): two signers are the same exactly when that record encodes identically, and
/// ordering is byte order on the public key, which is the only varying field. A
/// [`SignerHandle`] names one in text; it is a labeled non-certified digest and never
/// decides sameness.
///
/// Construction validates the key: a byte string that is not a curve point, or is a weak
/// (small-order) point, is refused, so no `SignerIdentity` exists for a key under which a
/// signature could verify for every message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignerIdentity {
    public_key: [u8; PUBLIC_KEY_LEN],
}

impl SignerIdentity {
    /// A signer identity from its Ed25519 public key.
    ///
    /// # Errors
    ///
    /// [`WireError::InvalidPublicKey`] when the bytes are not a valid, non-weak point.
    pub fn from_public_key(bytes: &[u8]) -> Result<Self, WireError> {
        let public_key: [u8; PUBLIC_KEY_LEN] =
            bytes.try_into().map_err(|_| WireError::InvalidPublicKey)?;
        let key = VerifyingKey::from_bytes(&public_key).map_err(|_| WireError::InvalidPublicKey)?;
        if key.is_weak() {
            return Err(WireError::InvalidPublicKey);
        }
        Ok(Self { public_key })
    }

    /// The Ed25519 public key.
    #[must_use]
    pub const fn public_key(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.public_key
    }

    /// The scheme. Always [`SignatureScheme::Ed25519`] in this build.
    #[must_use]
    pub const fn scheme(&self) -> SignatureScheme {
        SignatureScheme::Ed25519
    }

    /// The canonical record.
    #[must_use]
    pub fn to_value(&self) -> Value {
        record([
            ("public_key", Value::bytes(self.public_key.to_vec())),
            ("scheme", Value::text(self.scheme().as_str())),
        ])
    }

    /// Read a signer from its canonical record.
    ///
    /// # Errors
    ///
    /// [`WireError`] on a wrong shape, an unknown scheme, or an invalid key.
    pub fn from_value(value: &Value) -> Result<Self, WireError> {
        exact_fields(value, &["public_key", "scheme"])?;
        if text_field(value, "scheme")? != SignatureScheme::Ed25519.as_str() {
            return Err(WireError::UnknownScheme);
        }
        Self::from_public_key(bytes_field(value, "public_key")?)
    }

    /// The exact ADR-0013 identity.
    #[must_use]
    pub fn content_identity(&self) -> ContentIdentity {
        ContentIdentity::of(&self.to_value())
    }

    /// The `signer_` handle: BLAKE3 over the canonical record, under
    /// [`SIGNER_HANDLE_LABEL`].
    ///
    /// # Panics
    ///
    /// Never: the label is a constant inside the non-certified label class, and a hex
    /// digest is inside the handle class.
    #[must_use]
    pub fn handle(&self) -> SignerHandle {
        let label = NonCertifiedLabel::new(SIGNER_HANDLE_LABEL)
            .expect("the signer handle label is inside the non-certified label class");
        let digest = HashIdentity::compute::<Blake3Hasher>(
            &Value::bytes(self.content_identity().canonical_bytes().to_vec()),
            label,
        )
        .digest();
        SignerHandle::new(&format!("{SIGNER_HANDLE_PREFIX}{}", digest.to_token()))
            .expect("a hex digest is inside the handle class")
    }

    fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey::from_bytes(&self.public_key).expect("validated at construction")
    }
}

/// A `signer_` handle: how a signer is named in text. Never how sameness is decided.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignerHandle(String);

impl SignerHandle {
    /// The pattern this type accepts.
    pub const PATTERN: &'static str = "^signer_[A-Za-z0-9_-]+$";

    /// Parse a handle.
    ///
    /// # Errors
    ///
    /// [`WireError::Shape`] when the prefix is missing, the rest is empty, or it carries a
    /// character outside `[A-Za-z0-9_-]`.
    pub fn new(text: &str) -> Result<Self, WireError> {
        let rest = text
            .strip_prefix(SIGNER_HANDLE_PREFIX)
            .ok_or(WireError::Shape("signer handle prefix"))?;
        if rest.is_empty()
            || !rest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(WireError::Shape("signer handle character"));
        }
        Ok(Self(text.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SignerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Signatures.
// ---------------------------------------------------------------------------

/// An Ed25519 signature: 64 bytes, no interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signature([u8; SIGNATURE_LEN]);

impl Signature {
    /// A signature from its bytes.
    ///
    /// # Errors
    ///
    /// [`WireError::InvalidSignature`] when the length is not 64.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        bytes
            .try_into()
            .map(Self)
            .map_err(|_| WireError::InvalidSignature)
    }

    /// The bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; SIGNATURE_LEN] {
        &self.0
    }

    /// The text token, `ed25519:` then 128 lowercase hexadecimal digits.
    #[must_use]
    pub fn to_token(&self) -> String {
        format!("{SIGNATURE_TOKEN_PREFIX}{}", to_hex(&self.0))
    }

    /// Parse a text token.
    ///
    /// # Errors
    ///
    /// [`WireError::InvalidSignature`] on a missing prefix, a length other than exactly
    /// [`SIGNATURE_LEN`]` * 2` digits (checked first), or a non-lowercase-hex digit.
    pub fn from_token(token: &str) -> Result<Self, WireError> {
        let hex = token
            .strip_prefix(SIGNATURE_TOKEN_PREFIX)
            .ok_or(WireError::InvalidSignature)?;
        from_hex_exact::<SIGNATURE_LEN>(hex)
            .map(Self)
            .ok_or(WireError::InvalidSignature)
    }
}

/// The canonical bytes a signature over `artifact` covers (see the module docs).
#[must_use]
pub fn signing_message(
    kind: SignedArtifactKind,
    signer: &SignerIdentity,
    artifact: &ContentIdentity,
) -> Vec<u8> {
    record([
        (
            "artifact",
            Value::bytes(artifact.canonical_bytes().to_vec()),
        ),
        ("domain", Value::text(SIGNATURE_DOMAIN)),
        ("kind", Value::text(kind.as_str())),
        ("signer", signer.to_value()),
    ])
    .encode()
}

/// A signature over one artifact, with the kind and signer it claims.
///
/// This is the record that travels beside the artifact. Its canonical encoding
/// ([`encode`](Self::encode)) is the wire form; a verifier reads it back with
/// [`decode`](Self::decode) and checks it from those bytes alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactSignature {
    kind: SignedArtifactKind,
    signer: SignerIdentity,
    signature: Signature,
}

impl ArtifactSignature {
    /// Assemble a signature record from parts, for a caller that received them separately.
    /// Nothing is checked here; [`SignatureVerifier`] checks.
    #[must_use]
    pub const fn from_parts(
        kind: SignedArtifactKind,
        signer: SignerIdentity,
        signature: Signature,
    ) -> Self {
        Self {
            kind,
            signer,
            signature,
        }
    }

    /// The kind this signature claims.
    #[must_use]
    pub const fn kind(&self) -> SignedArtifactKind {
        self.kind
    }

    /// The signer this signature claims.
    #[must_use]
    pub const fn signer(&self) -> &SignerIdentity {
        &self.signer
    }

    /// The signature bytes.
    #[must_use]
    pub const fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The canonical record.
    #[must_use]
    pub fn to_value(&self) -> Value {
        record([
            ("kind", Value::text(self.kind.as_str())),
            ("signature", Value::bytes(self.signature.0.to_vec())),
            ("signer", self.signer.to_value()),
        ])
    }

    /// Read a signature record from its canonical value.
    ///
    /// # Errors
    ///
    /// [`WireError`] on any shape, kind, key, or length defect.
    pub fn from_value(value: &Value) -> Result<Self, WireError> {
        exact_fields(value, &["kind", "signature", "signer"])?;
        let kind = SignedArtifactKind::from_token(text_field(value, "kind")?)
            .ok_or(WireError::UnknownKind)?;
        let signer = SignerIdentity::from_value(field(value, "signer")?)?;
        let signature = Signature::from_bytes(bytes_field(value, "signature")?)?;
        Ok(Self {
            kind,
            signer,
            signature,
        })
    }

    /// The canonical wire bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.to_value().encode()
    }

    /// Read the canonical wire bytes.
    ///
    /// # Errors
    ///
    /// [`WireError::TooLarge`] for more than [`MAX_SIGNATURE_RECORD_LEN`] bytes (checked
    /// before any parsing), [`WireError::NotCanonical`] for bytes that are not a canonical
    /// value, otherwise as [`from_value`](Self::from_value).
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_SIGNATURE_RECORD_LEN {
            return Err(WireError::TooLarge);
        }
        Self::from_value(&Value::decode(bytes).map_err(WireError::NotCanonical)?)
    }
}

// ---------------------------------------------------------------------------
// Keys and entropy.
// ---------------------------------------------------------------------------

/// The capability through which key generation receives entropy (INV-005, ADR-0003).
///
/// Nothing in this crate reads the operating system's entropy: a key is a function of the
/// 32-byte seed this capability returns, and nothing else. A deployment passes an
/// operating-system source from a boundary crate; a test passes a fixed seed.
///
/// The seed travels in a [`Zeroizing`] wrapper and is moved, never copied, into key
/// derivation, so it is wiped when derivation returns on every path — success, a weak key,
/// or a refused duplicate mint.
pub trait KeyEntropy {
    /// Return a fresh 32-byte secret seed.
    ///
    /// # Errors
    ///
    /// [`EntropyUnavailable`] when the source cannot supply one. Minting then fails; it
    /// never falls back to a weaker source.
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable>;
}

/// The entropy capability could not supply a seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyUnavailable;

impl fmt::Display for EntropyUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the key-entropy capability could not supply a seed")
    }
}

impl core::error::Error for EntropyUnavailable {}

/// A local signing key and the identity it signs as.
///
/// The only way to obtain one is through [`SigningRegistry`]'s audited operations, and the
/// only way to sign with one is [`SigningRegistry::sign`], which refuses a signer that is
/// not in active standing. There is no `Clone` and no secret in `Debug`; the secret is
/// wiped on drop (`zeroize`).
pub struct LocalSigner {
    key: SigningKey,
    identity: SignerIdentity,
}

impl LocalSigner {
    /// Derive a key from `seed`, taken by value: the seed drops (and is wiped) when this
    /// returns, whichever path returns. `SigningKey` keeps its own zeroizing copy.
    fn from_seed(seed: Zeroizing<[u8; SEED_LEN]>) -> Result<Self, MintError> {
        let key = SigningKey::from_bytes(&seed);
        drop(seed);
        let identity = SignerIdentity::from_public_key(key.verifying_key().as_bytes())
            .map_err(|_| MintError::WeakKey)?;
        Ok(Self { key, identity })
    }

    /// The identity this key signs as.
    #[must_use]
    pub const fn identity(&self) -> &SignerIdentity {
        &self.identity
    }

    fn sign_message(&self, message: &[u8]) -> Signature {
        Signature(self.key.sign(message).to_bytes())
    }
}

impl fmt::Debug for LocalSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSigner")
            .field("identity", &self.identity.handle())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Registry: standing and audit.
// ---------------------------------------------------------------------------

/// Why a signer was revoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RevocationReason {
    /// The secret key is, or may be, known to someone else.
    Compromised,
    /// The secret key is lost (docs/09 §10, "Loss recovery").
    KeyLost,
}

impl RevocationReason {
    /// The stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compromised => "compromised",
            Self::KeyLost => "key-lost",
        }
    }
}

/// A signer's standing in a [`SigningRegistry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignerStanding {
    /// May sign; signatures verify.
    Active,
    /// Retired by rotation. May not sign; signatures it made still verify (plan §4.6).
    Rotated {
        /// The identity that replaced it.
        successor: SignerIdentity,
        /// The audit record of the rotation.
        record: u64,
    },
    /// Revoked. May not sign; signatures it made downgrade to unverified.
    Revoked {
        /// Why.
        reason: RevocationReason,
        /// The audit record of the revocation.
        record: u64,
    },
}

/// One audited signing-identity operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningEvent {
    /// A new identity was minted.
    Minted {
        /// The new identity.
        signer: SignerIdentity,
    },
    /// An identity was retired in favour of a successor, by the holder of the old key.
    Rotated {
        /// The retired identity.
        from: SignerIdentity,
        /// Its successor.
        to: SignerIdentity,
    },
    /// An identity was revoked.
    Revoked {
        /// The revoked identity.
        signer: SignerIdentity,
        /// Why.
        reason: RevocationReason,
    },
    /// A lost identity was superseded by a freshly minted one (docs/09 §10). The revocation
    /// and the mint are their own records; this one links them.
    Superseded {
        /// The lost identity.
        lost: SignerIdentity,
        /// The new identity.
        successor: SignerIdentity,
    },
}

/// An entry of the signing audit log: who did what, in which position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningAuditRecord {
    sequence: u64,
    actor: ActorId,
    event: SigningEvent,
}

impl SigningAuditRecord {
    /// Position in the log, from zero.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// The actor who performed the operation.
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// The operation.
    #[must_use]
    pub const fn event(&self) -> &SigningEvent {
        &self.event
    }

    /// The canonical record, for an audit-log content identity (plan §18.5).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let event = match &self.event {
            SigningEvent::Minted { signer } => {
                record([("op", Value::text("mint")), ("signer", signer.to_value())])
            }
            SigningEvent::Rotated { from, to } => record([
                ("from", from.to_value()),
                ("op", Value::text("rotate")),
                ("to", to.to_value()),
            ]),
            SigningEvent::Revoked { signer, reason } => record([
                ("op", Value::text("revoke")),
                ("reason", Value::text(reason.as_str())),
                ("signer", signer.to_value()),
            ]),
            SigningEvent::Superseded { lost, successor } => record([
                ("lost", lost.to_value()),
                ("op", Value::text("supersede")),
                ("successor", successor.to_value()),
            ]),
        };
        record([
            ("actor", self.actor.to_value()),
            ("event", event),
            ("sequence", Value::Nat(u128::from(self.sequence))),
        ])
    }

    /// Read a record from its canonical value — the inverse of
    /// [`to_value`](Self::to_value), for a persisted audit log (bn-1hape).
    ///
    /// # Errors
    ///
    /// [`WireError`] on any shape, token, actor, key, or sequence defect.
    pub fn from_value(value: &Value) -> Result<Self, WireError> {
        exact_fields(value, &["actor", "event", "sequence"])?;
        let actor =
            ActorId::new(text_field(value, "actor")?).map_err(|_| WireError::Shape("actor"))?;
        let sequence = match field(value, "sequence")? {
            Value::Nat(n) => u64::try_from(*n).map_err(|_| WireError::Shape("sequence"))?,
            _ => return Err(WireError::Shape("sequence")),
        };
        let event = field(value, "event")?;
        let signer_at = |key| SignerIdentity::from_value(field(event, key)?);
        let event = match text_field(event, "op")? {
            "mint" => {
                exact_fields(event, &["op", "signer"])?;
                SigningEvent::Minted {
                    signer: signer_at("signer")?,
                }
            }
            "rotate" => {
                exact_fields(event, &["from", "op", "to"])?;
                SigningEvent::Rotated {
                    from: signer_at("from")?,
                    to: signer_at("to")?,
                }
            }
            "revoke" => {
                exact_fields(event, &["op", "reason", "signer"])?;
                let reason = match text_field(event, "reason")? {
                    "compromised" => RevocationReason::Compromised,
                    "key-lost" => RevocationReason::KeyLost,
                    _ => return Err(WireError::Shape("revocation reason")),
                };
                SigningEvent::Revoked {
                    signer: signer_at("signer")?,
                    reason,
                }
            }
            "supersede" => {
                exact_fields(event, &["lost", "op", "successor"])?;
                SigningEvent::Superseded {
                    lost: signer_at("lost")?,
                    successor: signer_at("successor")?,
                }
            }
            _ => return Err(WireError::Shape("audit operation")),
        };
        Ok(Self {
            sequence,
            actor,
            event,
        })
    }
}

/// Why a persisted audit log could not be replayed into a registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The log is not a sequence of well-formed records.
    Wire(WireError),
    /// A record's sequence number is not its position.
    Sequence {
        /// The position.
        expected: u64,
        /// The number the record carries.
        found: u64,
    },
    /// A record is not a legal transition from the standing the earlier records built —
    /// a second mint of one key, a rotation of an inactive signer, a revocation of a
    /// revoked or unknown one, or a supersession that does not follow a key-lost revocation.
    IllegalTransition {
        /// The record's position.
        sequence: u64,
    },
}

/// Why [`SigningRegistry::restore`] refused a persisted seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreError {
    /// The seed derives a key the registry holds no record of. A persisted key is only
    /// ever one this registry minted; anything else is refused, not adopted.
    UnknownSigner,
    /// The seed derives a weak key.
    WeakKey,
}

/// Why minting failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MintError {
    /// The entropy capability supplied no seed.
    Entropy(EntropyUnavailable),
    /// The seed produced an identity this registry already holds. Refused rather than
    /// silently returning a second handle to one key.
    AlreadyMinted,
    /// The seed produced a weak public key (not reachable for a clamped scalar; kept total).
    WeakKey,
}

/// Why a standing change was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandingError {
    /// The registry holds no record of this signer.
    UnknownSigner,
    /// The operation needs an active signer and this one is not active.
    NotActive(SignerStanding),
    /// Minting the successor failed.
    Mint(MintError),
}

/// Why [`SigningRegistry::sign`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignRefusal {
    /// The registry holds no record of this signer.
    UnknownSigner,
    /// The signer is rotated or revoked.
    NotActive(SignerStanding),
}

/// Signer standing, plus the append-only audit log of every operation that changed it.
///
/// The registry holds no secret: a [`LocalSigner`] is returned to the caller (normally a
/// [`LocalKeyring`]), and verification reads only public state. That is what lets a
/// verifier hold a registry without holding a key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SigningRegistry {
    standing: BTreeMap<SignerIdentity, SignerStanding>,
    audit: Vec<SigningAuditRecord>,
}

impl SigningRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&mut self, actor: &ActorId, event: SigningEvent) -> u64 {
        let sequence = u64::try_from(self.audit.len()).expect("an audit log fits in u64");
        self.audit.push(SigningAuditRecord {
            sequence,
            actor: actor.clone(),
            event,
        });
        sequence
    }

    /// Mint a new identity from the entropy capability, and record it.
    ///
    /// # Errors
    ///
    /// [`MintError`] when entropy is unavailable or the identity already exists.
    pub fn mint(
        &mut self,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<LocalSigner, MintError> {
        let signer = LocalSigner::from_seed(entropy.seed().map_err(MintError::Entropy)?)?;
        if self.standing.contains_key(signer.identity()) {
            return Err(MintError::AlreadyMinted);
        }
        self.standing
            .insert(signer.identity().clone(), SignerStanding::Active);
        self.record(
            actor,
            SigningEvent::Minted {
                signer: signer.identity().clone(),
            },
        );
        Ok(signer)
    }

    /// Retire `current` in favour of a freshly minted successor. The caller proves
    /// possession of the old key by presenting it.
    ///
    /// # Errors
    ///
    /// [`StandingError`] when `current` is unknown or not active, or minting fails. Nothing
    /// is recorded on failure.
    pub fn rotate(
        &mut self,
        current: &LocalSigner,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<LocalSigner, StandingError> {
        self.require_active(current.identity())?;
        let successor = self.mint(actor, entropy).map_err(StandingError::Mint)?;
        let record = self.record(
            actor,
            SigningEvent::Rotated {
                from: current.identity().clone(),
                to: successor.identity().clone(),
            },
        );
        self.standing.insert(
            current.identity().clone(),
            SignerStanding::Rotated {
                successor: successor.identity().clone(),
                record,
            },
        );
        Ok(successor)
    }

    /// Revoke a signer. Every signature it made downgrades to unverified from now on.
    ///
    /// # Errors
    ///
    /// [`StandingError::UnknownSigner`], or [`StandingError::NotActive`] when it is
    /// already revoked. A rotated signer may be revoked.
    pub fn revoke(
        &mut self,
        signer: &SignerIdentity,
        actor: &ActorId,
        reason: RevocationReason,
    ) -> Result<u64, StandingError> {
        match self.standing.get(signer) {
            None => return Err(StandingError::UnknownSigner),
            Some(standing @ SignerStanding::Revoked { .. }) => {
                return Err(StandingError::NotActive(standing.clone()));
            }
            Some(_) => {}
        }
        let record = self.record(
            actor,
            SigningEvent::Revoked {
                signer: signer.clone(),
                reason,
            },
        );
        self.standing
            .insert(signer.clone(), SignerStanding::Revoked { reason, record });
        Ok(record)
    }

    /// Loss recovery (docs/09 §10): revoke `lost` as [`RevocationReason::KeyLost`], mint a
    /// successor, and link the two with a supersession record. A lost key is not recovered.
    ///
    /// # Errors
    ///
    /// [`StandingError`] when `lost` is unknown or already revoked, or minting fails.
    /// Minting runs first, so a failure records nothing.
    pub fn supersede_lost(
        &mut self,
        lost: &SignerIdentity,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<LocalSigner, StandingError> {
        match self.standing.get(lost) {
            None => return Err(StandingError::UnknownSigner),
            Some(standing @ SignerStanding::Revoked { .. }) => {
                return Err(StandingError::NotActive(standing.clone()));
            }
            Some(_) => {}
        }
        let successor = self.mint(actor, entropy).map_err(StandingError::Mint)?;
        self.revoke(lost, actor, RevocationReason::KeyLost)?;
        self.record(
            actor,
            SigningEvent::Superseded {
                lost: lost.clone(),
                successor: successor.identity().clone(),
            },
        );
        Ok(successor)
    }

    fn require_active(&self, signer: &SignerIdentity) -> Result<(), StandingError> {
        match self.standing.get(signer) {
            None => Err(StandingError::UnknownSigner),
            Some(SignerStanding::Active) => Ok(()),
            Some(other) => Err(StandingError::NotActive(other.clone())),
        }
    }

    /// Sign an artifact's canonical bytes as `kind`.
    ///
    /// # Errors
    ///
    /// [`SignRefusal`] when the signer is unknown to this registry, rotated, or revoked.
    pub fn sign(
        &self,
        signer: &LocalSigner,
        kind: SignedArtifactKind,
        artifact: &ContentIdentity,
    ) -> Result<ArtifactSignature, SignRefusal> {
        match self.standing.get(signer.identity()) {
            None => return Err(SignRefusal::UnknownSigner),
            Some(SignerStanding::Active) => {}
            Some(other) => return Err(SignRefusal::NotActive(other.clone())),
        }
        let message = signing_message(kind, signer.identity(), artifact);
        Ok(ArtifactSignature {
            kind,
            signer: signer.identity().clone(),
            signature: signer.sign_message(&message),
        })
    }

    /// A signer's standing, or `None` when this registry holds no record of it.
    #[must_use]
    pub fn standing(&self, signer: &SignerIdentity) -> Option<&SignerStanding> {
        self.standing.get(signer)
    }

    /// The audit log, oldest first.
    #[must_use]
    pub fn audit_log(&self) -> &[SigningAuditRecord] {
        &self.audit
    }

    /// The content identity of this registry's whole audit log: what a verifier compares
    /// against the authoritative head to know it holds every rotation and revocation.
    #[must_use]
    pub fn head(&self) -> RegistryHead {
        RegistryHead(ContentIdentity::of(&self.audit_value()))
    }

    /// The whole audit log as one canonical value: what a keystore persists, and what
    /// [`replay`](Self::replay) reads back.
    #[must_use]
    pub fn audit_value(&self) -> Value {
        Value::seq(self.audit.iter().map(SigningAuditRecord::to_value))
            .expect("a sequence of records is well-formed")
    }

    /// Rebuild a registry from a persisted audit log, re-deriving every standing from the
    /// records in order. The result's [`head`](Self::head) is the log's own identity, so a
    /// replayed registry is current exactly when the persisted log was.
    ///
    /// # Errors
    ///
    /// [`ReplayError`] on a malformed record, a sequence gap, or an illegal transition.
    pub fn replay(log: &Value) -> Result<Self, ReplayError> {
        let Value::Seq(records) = log else {
            return Err(ReplayError::Wire(WireError::Shape("audit log sequence")));
        };
        let mut registry = Self::new();
        for value in records {
            let record = SigningAuditRecord::from_value(value).map_err(ReplayError::Wire)?;
            registry.replay_record(record)?;
        }
        Ok(registry)
    }

    /// Apply one persisted record to this registry, as [`replay`](Self::replay) does for
    /// each record in order. This is the incremental form a store uses to replay a log one
    /// bounded record at a time, so no whole-log value is ever materialized (bn-1hape,
    /// cr-3l3n47). On an error the registry is unchanged.
    ///
    /// # Errors
    ///
    /// [`ReplayError::Sequence`] when the record's number is not the next position;
    /// [`ReplayError::IllegalTransition`] when it is not a legal transition.
    pub fn replay_record(&mut self, record: SigningAuditRecord) -> Result<(), ReplayError> {
        let expected = u64::try_from(self.audit.len()).expect("a log fits in u64");
        if record.sequence != expected {
            return Err(ReplayError::Sequence {
                expected,
                found: record.sequence,
            });
        }
        let illegal = ReplayError::IllegalTransition { sequence: expected };
        match &record.event {
            SigningEvent::Minted { signer } => {
                if self.standing.contains_key(signer) {
                    return Err(illegal);
                }
                self.standing.insert(signer.clone(), SignerStanding::Active);
            }
            SigningEvent::Rotated { from, to } => {
                if self.standing.get(from) != Some(&SignerStanding::Active)
                    || self.standing.get(to) != Some(&SignerStanding::Active)
                    || from == to
                {
                    return Err(illegal);
                }
                self.standing.insert(
                    from.clone(),
                    SignerStanding::Rotated {
                        successor: to.clone(),
                        record: expected,
                    },
                );
            }
            SigningEvent::Revoked { signer, reason } => match self.standing.get(signer) {
                None | Some(SignerStanding::Revoked { .. }) => return Err(illegal),
                Some(_) => {
                    self.standing.insert(
                        signer.clone(),
                        SignerStanding::Revoked {
                            reason: *reason,
                            record: expected,
                        },
                    );
                }
            },
            SigningEvent::Superseded { lost, successor } => {
                let lost_ok = matches!(
                    self.standing.get(lost),
                    Some(SignerStanding::Revoked {
                        reason: RevocationReason::KeyLost,
                        ..
                    })
                );
                if !lost_ok || !self.standing.contains_key(successor) {
                    return Err(illegal);
                }
            }
        }
        self.audit.push(record);
        Ok(())
    }

    /// Re-derive a key this registry already minted from its persisted seed. Not an audit
    /// event: restoring is reading a key back, not minting one. The seed is taken by value
    /// and wiped on every path.
    ///
    /// # Errors
    ///
    /// [`RestoreError::UnknownSigner`] when the registry holds no record of the derived
    /// identity; [`RestoreError::WeakKey`] for a weak key.
    pub fn restore(&self, seed: Zeroizing<[u8; SEED_LEN]>) -> Result<LocalSigner, RestoreError> {
        let signer = LocalSigner::from_seed(seed).map_err(|_| RestoreError::WeakKey)?;
        if self.standing.contains_key(signer.identity()) {
            Ok(signer)
        } else {
            Err(RestoreError::UnknownSigner)
        }
    }
}

/// The exact identity of a signing registry's audit log (ADR-0013: the canonical bytes of
/// every record, in order).
///
/// Revocation is only as fresh as the registry a verifier reads. A verifier is therefore
/// built with the head the authority publishes, and a registry whose log differs — one
/// missing a later revocation, for example — cannot vouch for any signer's standing
/// ([`UnverifiedReason::StandingStale`]). Distributing the authoritative head is follow-up
/// work (ADR-0054); until it exists, a caller that cannot name one cannot reach
/// [`Provenance::Verified`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryHead(ContentIdentity);

impl RegistryHead {
    /// The canonical bytes of the audit log this head names.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        self.0.canonical_bytes()
    }
}

/// The solo-developer default: one local key, minted on first use (plan §18.6).
#[derive(Debug, Default)]
pub struct LocalKeyring {
    signer: Option<LocalSigner>,
}

impl LocalKeyring {
    /// A keyring that holds no key yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { signer: None }
    }

    /// The local signer, minting and recording it on the first call. Later calls return the
    /// same key and draw no entropy.
    ///
    /// # Errors
    ///
    /// [`MintError`] on the first call when minting fails; the keyring stays empty.
    pub fn signer_or_mint(
        &mut self,
        registry: &mut SigningRegistry,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<&LocalSigner, MintError> {
        if self.signer.is_none() {
            self.signer = Some(registry.mint(actor, entropy)?);
        }
        Ok(self.signer.as_ref().expect("minted above"))
    }

    /// Rotate the local key: the registry retires it and this keyring holds the successor.
    ///
    /// # Errors
    ///
    /// [`StandingError::UnknownSigner`] when the keyring is empty, otherwise as
    /// [`SigningRegistry::rotate`]. The old key stays in place on failure.
    pub fn rotate(
        &mut self,
        registry: &mut SigningRegistry,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<&LocalSigner, StandingError> {
        let current = self.signer.as_ref().ok_or(StandingError::UnknownSigner)?;
        let successor = registry.rotate(current, actor, entropy)?;
        self.signer = Some(successor);
        Ok(self.signer.as_ref().expect("replaced above"))
    }

    /// The local signer, if one has been minted.
    #[must_use]
    pub const fn signer(&self) -> Option<&LocalSigner> {
        self.signer.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Allowed signers.
// ---------------------------------------------------------------------------

/// The trust root an intent bundle pins (plan §4.2.1, RFC 0037 A3): which signer may sign
/// which artifact kind. Local policy, not the signature, decides which signers count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AllowedSigners {
    entries: BTreeMap<SignerIdentity, BTreeSet<SignedArtifactKind>>,
}

impl AllowedSigners {
    /// An empty set: no signer is allowed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allow `signer` for `kinds`, in addition to anything it is already allowed.
    #[must_use]
    pub fn allow(
        mut self,
        signer: SignerIdentity,
        kinds: impl IntoIterator<Item = SignedArtifactKind>,
    ) -> Self {
        self.entries.entry(signer).or_default().extend(kinds);
        self
    }

    /// Whether `signer` may sign `kind`.
    #[must_use]
    pub fn permits(&self, signer: &SignerIdentity, kind: SignedArtifactKind) -> bool {
        self.entries
            .get(signer)
            .is_some_and(|kinds| kinds.contains(&kind))
    }

    /// The canonical value: a sequence of `{ kinds: Set(Text), signer }`, in signer order.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::seq(self.entries.iter().map(|(signer, kinds)| {
            record([
                (
                    "kinds",
                    Value::set(kinds.iter().map(|kind| Value::text(kind.as_str())))
                        .expect("a set of text values is well-formed"),
                ),
                ("signer", signer.to_value()),
            ])
        }))
        .expect("a sequence of records is well-formed")
    }

    /// Read an allowed-signers set from its canonical value.
    ///
    /// # Errors
    ///
    /// [`WireError`] on a wrong shape, an unknown kind, an invalid key, a repeated signer,
    /// or signers out of canonical order.
    pub fn from_value(value: &Value) -> Result<Self, WireError> {
        let Value::Seq(items) = value else {
            return Err(WireError::Shape("allowed-signers sequence"));
        };
        let mut entries = BTreeMap::new();
        let mut previous: Option<SignerIdentity> = None;
        for item in items {
            exact_fields(item, &["kinds", "signer"])?;
            let signer = SignerIdentity::from_value(field(item, "signer")?)?;
            if previous.as_ref().is_some_and(|p| p >= &signer) {
                return Err(WireError::Shape("allowed signers out of canonical order"));
            }
            let Value::Set(kinds) = field(item, "kinds")? else {
                return Err(WireError::Shape("kinds"));
            };
            let kinds = kinds
                .iter()
                .map(|kind| match kind {
                    Value::Text(text) => {
                        SignedArtifactKind::from_token(text).ok_or(WireError::UnknownKind)
                    }
                    _ => Err(WireError::Shape("kind")),
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            previous = Some(signer.clone());
            entries.insert(signer, kinds);
        }
        Ok(Self { entries })
    }

    /// The exact ADR-0013 identity an intent bundle pins.
    #[must_use]
    pub fn content_identity(&self) -> ContentIdentity {
        ContentIdentity::of(&self.to_value())
    }
}

// ---------------------------------------------------------------------------
// Verification.
// ---------------------------------------------------------------------------

/// The standing of a signer whose signature verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedStanding {
    /// Active.
    Active,
    /// Retired by rotation; the signature predates nothing this module can know, so the
    /// standing is reported rather than hidden (plan §4.6 keeps it verifiable).
    Rotated {
        /// The successor.
        successor: SignerIdentity,
    },
}

/// A signature that verified: who signed which kind, and the signer's standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProvenance {
    signer: SignerIdentity,
    kind: SignedArtifactKind,
    standing: VerifiedStanding,
}

impl VerifiedProvenance {
    /// The signer.
    #[must_use]
    pub const fn signer(&self) -> &SignerIdentity {
        &self.signer
    }

    /// The kind.
    #[must_use]
    pub const fn kind(&self) -> SignedArtifactKind {
        self.kind
    }

    /// The signer's standing.
    #[must_use]
    pub const fn standing(&self) -> &VerifiedStanding {
        &self.standing
    }
}

/// Why a signature did not verify (INV-008: each is a distinct outcome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnverifiedReason {
    /// No signature accompanied the artifact.
    Unsigned,
    /// The signature record's wire bytes are malformed.
    Malformed(WireError),
    /// The signature claims a different kind than the one being verified.
    KindMismatch {
        /// The kind the verifier asked for.
        expected: SignedArtifactKind,
        /// The kind the signature claims.
        found: SignedArtifactKind,
    },
    /// The signature does not verify for this artifact under the claimed key: a tampered
    /// payload, a forged signature, or the wrong key.
    SignatureMismatch,
    /// The signature is authentic, but its signer is revoked.
    SignerRevoked {
        /// Why the signer was revoked.
        reason: RevocationReason,
    },
    /// The signature is authentic, but the verifier's registry is not the authoritative one
    /// (its audit-log head differs), so it may be missing a revocation. Never read as
    /// active.
    StandingStale,
    /// The signature is authentic and the registry is current, but the registry holds no
    /// standing for this signer. An unknown signer is never presumed active.
    StandingUnknown,
    /// The signature is authentic, but the allowed-signers set does not permit this signer
    /// for this kind.
    SignerNotAllowed,
}

/// Typed unverified provenance: the artifact is kept, and says why it is not signed-for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnverifiedProvenance {
    kind: SignedArtifactKind,
    reason: UnverifiedReason,
}

impl UnverifiedProvenance {
    /// The kind that was being verified.
    #[must_use]
    pub const fn kind(&self) -> SignedArtifactKind {
        self.kind
    }

    /// Why it did not verify.
    #[must_use]
    pub const fn reason(&self) -> &UnverifiedReason {
        &self.reason
    }
}

/// The outcome of provenance verification. Total: never an error, never a boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// The signature verified.
    Verified(VerifiedProvenance),
    /// It did not; the artifact carries typed unverified provenance.
    Unverified(UnverifiedProvenance),
}

impl Provenance {
    /// The verified half, if any.
    #[must_use]
    pub const fn verified(&self) -> Option<&VerifiedProvenance> {
        match self {
            Self::Verified(verified) => Some(verified),
            Self::Unverified(_) => None,
        }
    }

    /// The unverified half, if any.
    #[must_use]
    pub const fn unverified(&self) -> Option<&UnverifiedProvenance> {
        match self {
            Self::Verified(_) => None,
            Self::Unverified(unverified) => Some(unverified),
        }
    }
}

/// The plan §4.2.1 CI acceptance check failed closed. The wire code is
/// `AcceptanceChainInvalid` (RFC 0026, RFC 0037 I3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceChainInvalid {
    reason: UnverifiedReason,
}

impl AcceptanceChainInvalid {
    /// Why.
    #[must_use]
    pub const fn reason(&self) -> &UnverifiedReason {
        &self.reason
    }
}

impl fmt::Display for AcceptanceChainInvalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "acceptance chain invalid: {:?}", self.reason)
    }
}

impl core::error::Error for AcceptanceChainInvalid {}

/// Verifies signatures against an allowed-signers set and a registry's standing.
///
/// Reads public state only. Checks run in a fixed order: presence, wire form, kind,
/// cryptography (Ed25519 strict verification), registry freshness, known standing,
/// revocation, then the allowed set — so a statement about a signer's standing is only ever
/// made about an authentic signature, and only from a registry that is the authority's.
/// `Verified` requires an authoritative registry entry for the signer.
#[derive(Debug, Clone, Copy)]
pub struct SignatureVerifier<'a> {
    allowed: &'a AllowedSigners,
    registry: &'a SigningRegistry,
    current: bool,
}

impl<'a> SignatureVerifier<'a> {
    /// A verifier over `allowed` and `registry`, which is current only if its audit-log
    /// head equals `authoritative`.
    #[must_use]
    pub fn new(
        allowed: &'a AllowedSigners,
        registry: &'a SigningRegistry,
        authoritative: &RegistryHead,
    ) -> Self {
        Self {
            allowed,
            registry,
            current: registry.head() == *authoritative,
        }
    }

    fn check(
        &self,
        kind: SignedArtifactKind,
        artifact: &ContentIdentity,
        signature: Option<&ArtifactSignature>,
    ) -> Result<VerifiedProvenance, UnverifiedReason> {
        let signature = signature.ok_or(UnverifiedReason::Unsigned)?;
        if signature.kind != kind {
            return Err(UnverifiedReason::KindMismatch {
                expected: kind,
                found: signature.kind,
            });
        }
        let message = signing_message(kind, &signature.signer, artifact);
        let raw = ed25519_dalek::Signature::from_bytes(&signature.signature.0);
        signature
            .signer
            .verifying_key()
            .verify_strict(&message, &raw)
            .map_err(|_| UnverifiedReason::SignatureMismatch)?;
        if !self.current {
            return Err(UnverifiedReason::StandingStale);
        }
        let standing = match self.registry.standing(&signature.signer) {
            None => return Err(UnverifiedReason::StandingUnknown),
            Some(SignerStanding::Revoked { reason, .. }) => {
                return Err(UnverifiedReason::SignerRevoked { reason: *reason });
            }
            Some(SignerStanding::Rotated { successor, .. }) => VerifiedStanding::Rotated {
                successor: successor.clone(),
            },
            Some(SignerStanding::Active) => VerifiedStanding::Active,
        };
        if !self.allowed.permits(&signature.signer, kind) {
            return Err(UnverifiedReason::SignerNotAllowed);
        }
        Ok(VerifiedProvenance {
            signer: signature.signer.clone(),
            kind,
            standing,
        })
    }

    /// Verify a signature record. Anything short of a verified signature is typed
    /// unverified provenance, never a pass and never an error.
    #[must_use]
    pub fn verify(
        &self,
        kind: SignedArtifactKind,
        artifact: &ContentIdentity,
        signature: Option<&ArtifactSignature>,
    ) -> Provenance {
        match self.check(kind, artifact, signature) {
            Ok(verified) => Provenance::Verified(verified),
            Err(reason) => Provenance::Unverified(UnverifiedProvenance { kind, reason }),
        }
    }

    /// Verify from wire bytes: decode, then [`verify`](Self::verify). Malformed bytes are
    /// [`UnverifiedReason::Malformed`].
    #[must_use]
    pub fn verify_encoded(
        &self,
        kind: SignedArtifactKind,
        artifact: &ContentIdentity,
        encoded: Option<&[u8]>,
    ) -> Provenance {
        match encoded.map(ArtifactSignature::decode).transpose() {
            Ok(signature) => self.verify(kind, artifact, signature.as_ref()),
            Err(error) => Provenance::Unverified(UnverifiedProvenance {
                kind,
                reason: UnverifiedReason::Malformed(error),
            }),
        }
    }

    /// The plan §4.2.1 CI acceptance check: the same verification, failing closed.
    ///
    /// # Errors
    ///
    /// [`AcceptanceChainInvalid`] for every outcome other than a verified signature,
    /// including an absent one.
    pub fn verify_for_ci_acceptance(
        &self,
        kind: SignedArtifactKind,
        artifact: &ContentIdentity,
        encoded: Option<&[u8]>,
    ) -> Result<VerifiedProvenance, AcceptanceChainInvalid> {
        match self.verify_encoded(kind, artifact, encoded) {
            Provenance::Verified(verified) => Ok(verified),
            Provenance::Unverified(unverified) => Err(AcceptanceChainInvalid {
                reason: unverified.reason,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(hex: &str) -> Zeroizing<[u8; SEED_LEN]> {
        Zeroizing::new(from_hex_exact::<SEED_LEN>(hex).expect("64 hex digits"))
    }

    /// RFC 8032 §7.1 TEST 1 and TEST 2: the primitive this module signs with is Ed25519 as
    /// specified, byte for byte. A feature or version change that altered it fails here.
    #[test]
    fn ed25519_matches_the_rfc_8032_test_vectors() {
        let vectors: [(&str, &str, &[u8], &str); 2] = [
            (
                "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                b"",
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            ),
            (
                "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
                "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                &[0x72],
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            ),
        ];
        for (secret, public, message, signature) in vectors {
            let signer = LocalSigner::from_seed(seed(secret)).expect("valid seed");
            assert_eq!(to_hex(signer.identity().public_key()), public);
            let signed = signer.sign_message(message);
            assert_eq!(to_hex(signed.as_bytes()), signature);
            signer
                .identity()
                .verifying_key()
                .verify_strict(
                    message,
                    &ed25519_dalek::Signature::from_bytes(signed.as_bytes()),
                )
                .expect("the published signature verifies");
        }
    }

    #[test]
    fn an_overlong_token_or_record_is_refused_before_decoding() {
        let long = format!("{SIGNATURE_TOKEN_PREFIX}{}", "a".repeat(1 << 20));
        assert_eq!(
            Signature::from_token(&long),
            Err(WireError::InvalidSignature)
        );
        assert_eq!(
            from_hex_exact::<SIGNATURE_LEN>(&"ab".repeat(SIGNATURE_LEN + 1)),
            None
        );
        let oversize = vec![0u8; MAX_SIGNATURE_RECORD_LEN + 1];
        assert_eq!(
            ArtifactSignature::decode(&oversize),
            Err(WireError::TooLarge)
        );
    }

    #[test]
    fn a_signature_record_fits_the_decode_bound() {
        let signer = LocalSigner::from_seed(Zeroizing::new([7u8; SEED_LEN])).expect("seed");
        let record = ArtifactSignature {
            kind: SignedArtifactKind::IntentBundle,
            signer: signer.identity().clone(),
            signature: signer.sign_message(b"bound"),
        };
        let encoded = record.encode();
        assert!(
            encoded.len() * 2 <= MAX_SIGNATURE_RECORD_LEN,
            "{}",
            encoded.len()
        );
        assert_eq!(ArtifactSignature::decode(&encoded), Ok(record));
    }

    #[test]
    fn signature_tokens_round_trip_and_reject_malformed_text() {
        let signature = Signature([0xab; SIGNATURE_LEN]);
        let token = signature.to_token();
        assert_eq!(
            token.len(),
            SIGNATURE_TOKEN_PREFIX.len() + 2 * SIGNATURE_LEN
        );
        assert_eq!(Signature::from_token(&token), Ok(signature));
        for bad in [
            "",
            "ed25519:",
            &token[..token.len() - 1],
            &token.to_uppercase(),
            &token.replace("ed25519:", "ssh-ed25519:"),
        ] {
            assert_eq!(
                Signature::from_token(bad),
                Err(WireError::InvalidSignature),
                "{bad}"
            );
        }
    }

    #[test]
    fn weak_and_non_point_public_keys_are_not_signer_identities() {
        // The identity element (small order) and a y-coordinate with no curve point.
        let mut identity_point = [0u8; PUBLIC_KEY_LEN];
        identity_point[0] = 1;
        assert_eq!(
            SignerIdentity::from_public_key(&identity_point),
            Err(WireError::InvalidPublicKey)
        );
        assert_eq!(
            SignerIdentity::from_public_key(&[0u8; 31]),
            Err(WireError::InvalidPublicKey)
        );
    }
}
