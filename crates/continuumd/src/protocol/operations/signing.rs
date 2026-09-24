//! The `signing` namespace: request and response structs for its 6 operations
//! (protocol 3.8, bn-3glnv).
//!
//! Plan §18.6 says identities "are minted through audited daemon operations", and these
//! are they. A signer travels as a [`SignerHandle`] and a public key; no struct here carries
//! a secret, and none can: the daemon holds the key and agents never do (INV-015, RFC 0032
//! "Signing"). Binary values — a public key, a signature record, an audit record — travel as
//! `Bytes`, the canonical encoding ADR-0054 D4 fixes, so a verifier checks them from those
//! bytes alone. The semantics are `rule signing.identities` and `rule
//! signing.verification`.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `signing.mint`.
    struct SigningMintRequest {
        /// The kinds local policy allows the new signer to sign. Non-empty.
        kinds: list<SignedArtifactKind> required;
    }
}

protocol_struct! {
    /// The `response` body of `signing.mint`.
    struct SigningMintResponse {
        /// The new signer.
        signer: SignerHandle required;
        /// Its 32-byte Ed25519 public key.
        public_key: Bytes required;
        /// The BLAKE3 digest of the registry head after the mint.
        registry_head: Bytes required;
    }
}

protocol_struct! {
    /// The `request` body of `signing.rotate`.
    struct SigningRotateRequest {
        /// The daemon's held signer, which the rotation retires.
        signer: SignerHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `signing.rotate`.
    struct SigningRotateResponse {
        /// The retired signer. Its signatures still verify, as rotated.
        retired: SignerHandle required;
        /// The successor, now the daemon's held signer.
        successor: SignerHandle required;
        /// The successor's 32-byte Ed25519 public key.
        public_key: Bytes required;
        /// The BLAKE3 digest of the registry head after the rotation.
        registry_head: Bytes required;
    }
}

protocol_struct! {
    /// The `request` body of `signing.revoke`.
    struct SigningRevokeRequest {
        /// The signer to revoke.
        signer: SignerHandle required;
        /// Why.
        reason: RevocationReason required;
    }
}

protocol_struct! {
    /// The `response` body of `signing.revoke`.
    struct SigningRevokeResponse {
        /// The revoked signer.
        signer: SignerHandle required;
        /// The successor a `key-lost` revocation of the held key minted, or null.
        successor: SignerHandle nullable;
        /// The BLAKE3 digest of the registry head after the revocation.
        registry_head: Bytes required;
    }
}

protocol_struct! {
    /// The `request` body of `signing.registry`. It names nothing.
    struct SigningRegistryRequest {
    }
}

protocol_struct! {
    /// The `response` body of `signing.registry`.
    struct SigningRegistryResponse {
        /// Every signer the registry holds standing for, in name order.
        signers: list<SignerHandle> required;
        /// Every audit record, oldest first, each the canonical encoding of one
        /// record: the revocation records a verifier needs.
        log: list<Bytes> required;
        /// The BLAKE3 digest of the registry head.
        registry_head: Bytes required;
        /// The local allowed-signers set, one canonical entry per signer, in
        /// signer order.
        allowed: list<Bytes> required;
    }
}

protocol_struct! {
    /// The `request` body of `signing.verify`.
    struct SigningVerifyRequest {
        /// The kind to verify as.
        kind: SignedArtifactKind required;
        /// The artifact's bytes, exactly as they were signed.
        artifact: Bytes required;
        /// The canonical signature record; absent is `unsigned`.
        signature: Bytes optional;
    }
}

protocol_struct! {
    /// The `response` body of `signing.verify`.
    struct SigningVerifyResponse {
        /// The typed outcome.
        outcome: SignatureOutcome required;
        /// The signer, present exactly when `outcome` is `verified`.
        signer: SignerHandle optional;
        /// The signer's successor, present exactly when a verified signer is rotated.
        successor: SignerHandle optional;
    }
}

protocol_struct! {
    /// The `request` body of `signing.sign_pack`.
    struct SigningSignPackRequest {
        /// The domain pack's bytes, exactly as they will be verified.
        pack: Bytes required;
    }
}

protocol_struct! {
    /// The `response` body of `signing.sign_pack`.
    struct SigningSignPackResponse {
        /// The canonical signature record over the pack.
        signature: Bytes required;
        /// The signer that signed it: the daemon's held signer.
        signer: SignerHandle required;
    }
}
