//! Differential: `continuum_evidence::signing`'s canonical records against an independent
//! construction through `continuum_value` (bn-2ee4c, ADR-0054 D4/D5).
//!
//! The signing module builds three canonical byte strings: the signer record, the signed
//! message, and the signature record. Each is rebuilt here directly from
//! `continuum_value::value::Value` and `continuum_value::identity`, following ADR-0054's
//! text rather than the module's code, and the two must agree byte for byte. A drift in
//! either the module's envelope or the value encoder that the other does not share fails
//! here. The oracle is `continuum-value`'s canonical encoder and hasher; the subject is
//! `continuum-evidence`'s signing records.

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    EntropyUnavailable, KeyEntropy, SEED_LEN, SIGNATURE_DOMAIN, SIGNER_HANDLE_LABEL,
    SignedArtifactKind, SigningRegistry, Zeroizing, signing_message,
};
use continuum_value::identity::{Blake3Hasher, ContentIdentity, HashIdentity, NonCertifiedLabel};
use continuum_value::value::{Name, Value};

struct Seed(u8);

impl KeyEntropy for Seed {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Ok(Zeroizing::new([self.0; SEED_LEN]))
    }
}

fn record(fields: &[(&str, Value)]) -> Value {
    Value::record(
        fields
            .iter()
            .map(|(key, value)| (Name::new(key).expect("name"), value.clone())),
    )
    .expect("record")
}

#[test]
fn signing_records_match_an_independent_continuum_value_construction() {
    let mut registry = SigningRegistry::new();
    let signer = registry
        .mint(
            &ActorId::new("human:solo-dev").expect("actor"),
            &mut Seed(21),
        )
        .expect("mint");
    let identity = signer.identity();

    // Signer record (ADR-0054 D5).
    let oracle_signer = record(&[
        ("public_key", Value::bytes(identity.public_key().to_vec())),
        ("scheme", Value::text("ed25519")),
    ]);
    assert_eq!(identity.to_value().encode(), oracle_signer.encode());
    assert_eq!(
        identity.content_identity(),
        ContentIdentity::of(&oracle_signer)
    );

    // Signer handle: BLAKE3 over the canonical record, under the declared label.
    let label = NonCertifiedLabel::new(SIGNER_HANDLE_LABEL).expect("label");
    let digest = HashIdentity::compute::<Blake3Hasher>(
        &Value::bytes(
            ContentIdentity::of(&oracle_signer)
                .canonical_bytes()
                .to_vec(),
        ),
        label,
    )
    .digest();
    assert_eq!(
        identity.handle().as_str(),
        format!("signer_{}", digest.to_token())
    );

    // Signed message and signature record (ADR-0054 D4), for every kind.
    let artifact = ContentIdentity::of(&Value::text("differential artifact"));
    for kind in SignedArtifactKind::ALL {
        let oracle_message = record(&[
            (
                "artifact",
                Value::bytes(artifact.canonical_bytes().to_vec()),
            ),
            ("domain", Value::text(SIGNATURE_DOMAIN)),
            ("kind", Value::text(kind.as_str())),
            ("signer", oracle_signer.clone()),
        ]);
        assert_eq!(
            signing_message(kind, identity, &artifact),
            oracle_message.encode()
        );

        let signature = registry.sign(&signer, kind, &artifact).expect("sign");
        let oracle_record = record(&[
            ("kind", Value::text(kind.as_str())),
            (
                "signature",
                Value::bytes(signature.signature().as_bytes().to_vec()),
            ),
            ("signer", oracle_signer.clone()),
        ]);
        assert_eq!(signature.encode(), oracle_record.encode());
    }
}
