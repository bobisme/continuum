//! The signed intent bundle's wire form (plan §4.2.1, RFC 0037, `rule intent.bundles`,
//! protocol 3.8, bn-3glnv).
//!
//! > Registries converge through signed, content-addressed **intent bundles** (`inb_*`): an
//! > export of one or more contracts with their acceptance records and policy tables.
//! >
//! > — plan §4.2.1
//!
//! # The layout, and why it is framed
//!
//! A bundle is untrusted input (INV-016), so its reader is built so that no step decodes
//! anything before the shape around it is checked, and no step allocates in proportion to a
//! count it has not bounded (review cr-3e3t1j, cr-3l3n47, and the bn-3glnv pre-review pass).
//! Every structural layer is a **frame list**, never a general value:
//!
//! ```text
//! frames      = u32 count, then count × (u32 length, that many bytes)          (big-endian)
//! signed      = frames(2):  body, signature
//! body        = frames(4):  domain = "continuum.intent-bundle.v1",
//!                           allowed_signers = frames(≤ 256 × ≤ 512 B),
//!                           contracts       = frames(≤ 64 × ≤ 256 KiB),
//!                           links           = frames(≤ 4096 × ≤ 512 B)
//! contract    = frames(3):  in_ handle (≤ 256 B), contract JSON, record JSON (≤ 8 KiB)
//! allowed     = AllowedSigners::entry_value, canonically encoded (one signer, its kinds)
//! link        = SignerLink::to_value, canonically encoded (one key-attested transition)
//! ```
//!
//! A frame list is read by checking its count against its bound, or its exact arity, and
//! against the bytes left (four bytes of length per item) **before** anything is allocated,
//! and each item's length against its own bound before the item is touched. The only general
//! decoder that runs is `Value::decode`, on one allowed-signers entry or one signer link at a
//! time, each at most 512 B, and each must re-encode to exactly its input. A contract's
//! JSON is read by `IntentContract::decode` later, one entry at a time. A frame list has one
//! spelling for its items, so a bundle has one spelling and its `inb_` identity is a
//! function of its content (RFC 0037, "Bundle contents and identity").
//!
//! The signature covers [`signed_bytes_identity`] of the body bytes, the same wrapping a
//! receipt's signature uses, so the body — contracts, records, the pinned allowed-signers
//! set, and the signer links — is signed as one artifact of kind `intent-bundle`. Each
//! link also carries the signatures of the keys it concerns, so its standing change is
//! adopted on those keys' word and never on the bundle signer's.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use continuum_evidence::signing::{
    AllowedSigners, MAX_LINK_RECORD_LEN, MAX_SIGNATURE_RECORD_LEN, SignerLink, WireError,
};
use continuum_value::identity::ContentIdentity;
use continuum_value::value::Value;

use crate::protocol::scalar::IntentHandle;

/// The domain string every bundle body carries.
pub const BUNDLE_DOMAIN: &str = "continuum.intent-bundle.v1";

/// The largest signed bundle, in bytes, the daemon reads or writes.
pub const MAX_BUNDLE_LEN: usize = 4 << 20;

/// The most contracts one bundle carries.
pub const MAX_BUNDLE_CONTRACTS: usize = 64;

/// The largest encoded contract entry (contract and record together), in bytes.
pub const MAX_CONTRACT_ENTRY_LEN: usize = 256 << 10;

/// The largest registry record JSON inside a contract entry, in bytes.
pub const MAX_RECORD_JSON_LEN: usize = 8 << 10;

/// The most signers one pinned allowed-signers set names.
pub const MAX_ALLOWED_SIGNERS: usize = 256;

/// The largest encoded allowed-signers entry, in bytes.
pub const MAX_ALLOWED_ENTRY_LEN: usize = 512;

/// The most audit records a registry holds, and so the most a bundle carries.
pub const MAX_AUDIT_RECORDS: usize = 4096;

/// The most signer links one bundle carries.
pub const MAX_BUNDLE_LINKS: usize = 4096;

/// The longest `in_` handle a contract entry carries, in bytes.
pub const MAX_INTENT_HANDLE_LEN: usize = 256;

/// The longest `inb_` handle the daemon holds a bundle under, in bytes: the custody
/// records every held bundle by its handle (bn-3snfi), so a handle is bounded before a
/// bundle is held. The deployment's content identifier derives it; a Blake3 handle is 68.
pub const MAX_BUNDLE_HANDLE_LEN: usize = 256;

/// Why bytes are not a signed bundle. Carries no byte of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleError {
    /// The input is longer than [`MAX_BUNDLE_LEN`]; it was not read.
    TooLarge,
    /// A layer is not a canonical value of the declared shape.
    Shape(&'static str),
    /// A frame list is truncated, over its bound, or carries trailing bytes.
    Frames(&'static str),
    /// Contracts are repeated or out of canonical order.
    Order,
    /// An entry's content is not what its field declares.
    Entry(&'static str),
}

impl core::fmt::Display for BundleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("the bundle exceeds its size bound"),
            Self::Shape(what) => write!(f, "the bundle is not canonical at {what}"),
            Self::Frames(what) => write!(f, "a frame list is malformed at {what}"),
            Self::Order => f.write_str("bundle contracts are repeated or out of order"),
            Self::Entry(what) => write!(f, "a bundle entry is malformed at {what}"),
        }
    }
}

impl core::error::Error for BundleError {}

/// The ADR-0013 identity a signature over `bytes` covers: the bytes as one canonical
/// `Bytes` value (ADR-0054 D4). Receipts, bundle bodies, and domain packs are all signed
/// over this wrapping, so the signer and every verifier call one function.
#[must_use]
pub fn signed_bytes_identity(bytes: &[u8]) -> ContentIdentity {
    ContentIdentity::of(&Value::bytes(bytes.to_vec()))
}

/// One contract a bundle exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleContract {
    /// The contract's `in_*` identity, as the exporter declared it (RFC 0037 I5 recomputes
    /// it on import).
    pub intent: IntentHandle,
    /// The contract's canonical artifact bytes (`schemas/intent-contract.schema.json`).
    pub contract: Vec<u8>,
    /// The registry record's canonical JSON (`schemas/intent-registry-record.schema.json`).
    pub record: Vec<u8>,
}

/// A bundle's body: what the signature covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleBody {
    /// The allowed-signers set the bundle pins. It narrows local policy and never widens
    /// it (RFC 0037 A3).
    pub allowed: AllowedSigners,
    /// The exported contracts, in strictly ascending `in_*` order.
    pub contracts: Vec<BundleContract>,
    /// The key-attested rotations and revocations the exporter knows, oldest first: the
    /// revocation records a verifier needs (ADR-0054 follow-up 4).
    pub links: Vec<SignerLink>,
}

/// A decoded signed bundle. Built only by [`decode_signed`], so its decoded `body` is always
/// the decoding of the `body_bytes` the signature covers, and its
/// [`content`](Self::content) is exactly the bytes it was decoded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedBundle {
    /// The signed bundle's bytes, shared: the daemon's custody records these very bytes
    /// (bn-3snfi) without copying them.
    content: Arc<[u8]>,
    body_bytes: Range<usize>,
    body: BundleBody,
    signature: Range<usize>,
}

impl SignedBundle {
    /// The exact body bytes the signature covers.
    #[must_use]
    pub fn body_bytes(&self) -> &[u8] {
        &self.content[self.body_bytes.clone()]
    }

    /// The signed bundle's bytes, exactly as decoded: `encode_signed(body_bytes,
    /// signature)`.
    #[must_use]
    pub const fn content(&self) -> &Arc<[u8]> {
        &self.content
    }

    /// The body, decoded from [`body_bytes`](Self::body_bytes).
    #[must_use]
    pub const fn body(&self) -> &BundleBody {
        &self.body
    }

    /// The encoded signature record, at most [`MAX_SIGNATURE_RECORD_LEN`] bytes. Decoded and
    /// checked by the verifier, never here.
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.content[self.signature.clone()]
    }

    /// The bytes this bundle charges against the held-bundle bound: its whole content,
    /// which is what the custody records.
    #[must_use]
    pub fn charged_len(&self) -> usize {
        self.content.len()
    }
}

fn frame(items: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = items.iter().map(|item| 4 + item.len()).sum();
    let mut out = Vec::with_capacity(4 + total);
    out.extend_from_slice(&u32::try_from(items.len()).expect("bounded").to_be_bytes());
    for item in items {
        out.extend_from_slice(&u32::try_from(item.len()).expect("bounded").to_be_bytes());
        out.extend_from_slice(item);
    }
    out
}

fn read_u32(bytes: &[u8], at: usize) -> Option<usize> {
    let word: [u8; 4] = bytes.get(at..at.checked_add(4)?)?.try_into().ok()?;
    usize::try_from(u32::from_be_bytes(word)).ok()
}

/// Split a frame list. The count is checked against `max_count` and against the bytes left
/// before the index is allocated, and each length against `max_item` before its slice is
/// taken, so nothing here allocates in proportion to an unchecked number.
fn unframe<'a>(
    bytes: &'a [u8],
    max_count: usize,
    max_item: usize,
    what: &'static str,
) -> Result<Vec<&'a [u8]>, BundleError> {
    let count = read_u32(bytes, 0).ok_or(BundleError::Frames(what))?;
    let rest = bytes.len() - 4;
    if count > max_count || count.checked_mul(4).is_none_or(|need| need > rest) {
        return Err(BundleError::Frames(what));
    }
    let mut items = Vec::with_capacity(count);
    let mut at = 4;
    for _ in 0..count {
        let len = read_u32(bytes, at).ok_or(BundleError::Frames(what))?;
        at += 4;
        if len > max_item {
            return Err(BundleError::Frames(what));
        }
        let end = at.checked_add(len).ok_or(BundleError::Frames(what))?;
        items.push(bytes.get(at..end).ok_or(BundleError::Frames(what))?);
        at = end;
    }
    if at != bytes.len() {
        return Err(BundleError::Frames(what));
    }
    Ok(items)
}

/// Split a frame list of exactly `arity` items, each at most `max_item` bytes.
fn unframe_exact<'a>(
    bytes: &'a [u8],
    arity: usize,
    max_item: usize,
    what: &'static str,
) -> Result<Vec<&'a [u8]>, BundleError> {
    let items = unframe(bytes, arity, max_item, what)?;
    if items.len() != arity {
        return Err(BundleError::Frames(what));
    }
    Ok(items)
}

/// Decode one small item as a canonical value, refusing any other spelling of it. Called
/// only on items whose length a frame bound already capped.
fn canonical(bytes: &[u8], what: &'static str) -> Result<Value, BundleError> {
    let value = Value::decode(bytes).map_err(|_| BundleError::Shape(what))?;
    if value.encode() != bytes {
        return Err(BundleError::Shape(what));
    }
    Ok(value)
}

impl BundleBody {
    /// The body's canonical bytes: what the signature covers.
    ///
    /// # Panics
    ///
    /// Never for a body within the bounds the exporter checks; every frame length fits
    /// `u32`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let allowed: Vec<Vec<u8>> = self
            .allowed
            .iter()
            .map(|(signer, kinds)| AllowedSigners::entry_value(signer, kinds).encode())
            .collect();
        let contracts: Vec<Vec<u8>> = self
            .contracts
            .iter()
            .map(|entry| {
                frame(&[
                    entry.intent.as_str().as_bytes().to_vec(),
                    entry.contract.clone(),
                    entry.record.clone(),
                ])
            })
            .collect();
        let links: Vec<Vec<u8>> = self.links.iter().map(SignerLink::encode).collect();
        frame(&[
            BUNDLE_DOMAIN.as_bytes().to_vec(),
            frame(&allowed),
            frame(&contracts),
            frame(&links),
        ])
    }

    fn decode(bytes: &[u8]) -> Result<Self, BundleError> {
        let layers = unframe_exact(bytes, 4, MAX_BUNDLE_LEN, "body")?;
        if layers[0] != BUNDLE_DOMAIN.as_bytes() {
            return Err(BundleError::Shape("domain"));
        }

        let allowed = unframe(
            layers[1],
            MAX_ALLOWED_SIGNERS,
            MAX_ALLOWED_ENTRY_LEN,
            "allowed_signers",
        )?;
        let allowed = AllowedSigners::from_entries(allowed.into_iter().map(|item| {
            let value =
                canonical(item, "allowed entry").map_err(|_| WireError::Shape("allowed entry"))?;
            AllowedSigners::entry_from_value(&value)
        }))
        .map_err(|_| BundleError::Entry("allowed_signers"))?;

        let contracts = unframe(
            layers[2],
            MAX_BUNDLE_CONTRACTS,
            MAX_CONTRACT_ENTRY_LEN,
            "contracts",
        )?;
        let mut entries: Vec<BundleContract> = Vec::with_capacity(contracts.len());
        for item in contracts {
            let [intent, contract, record_json] = <[&[u8]; 3]>::try_from(unframe_exact(
                item,
                3,
                MAX_CONTRACT_ENTRY_LEN,
                "contract entry",
            )?)
            .map_err(|_| BundleError::Frames("contract entry"))?;
            if intent.len() > MAX_INTENT_HANDLE_LEN || record_json.len() > MAX_RECORD_JSON_LEN {
                return Err(BundleError::Entry("contract entry"));
            }
            let intent = core::str::from_utf8(intent)
                .ok()
                .and_then(|text| IntentHandle::new(text).ok())
                .ok_or(BundleError::Entry("intent"))?;
            if entries.last().is_some_and(|last| last.intent >= intent) {
                return Err(BundleError::Order);
            }
            entries.push(BundleContract {
                intent,
                contract: contract.to_vec(),
                record: record_json.to_vec(),
            });
        }

        let links = unframe(layers[3], MAX_BUNDLE_LINKS, MAX_LINK_RECORD_LEN, "links")?
            .into_iter()
            .map(|item| SignerLink::decode(item).map_err(|_| BundleError::Entry("link")))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            allowed,
            contracts: entries,
            links,
        })
    }

    /// The exported contract `intent` names, by binary search over the canonical order.
    #[must_use]
    pub fn contract(&self, intent: &IntentHandle) -> Option<&BundleContract> {
        self.contracts
            .binary_search_by(|entry| entry.intent.cmp(intent))
            .ok()
            .map(|at| &self.contracts[at])
    }

    /// The distinct `in_*` identities the body exports.
    #[must_use]
    pub fn intents(&self) -> BTreeSet<&IntentHandle> {
        self.contracts.iter().map(|entry| &entry.intent).collect()
    }
}

/// The signed bundle's bytes: the two-item frame list `body, signature`.
#[must_use]
pub fn encode_signed(body_bytes: &[u8], signature: &[u8]) -> Vec<u8> {
    frame(&[body_bytes.to_vec(), signature.to_vec()])
}

/// Read a signed bundle. The length is checked before any byte is read, and every layer's
/// shape before anything inside it is decoded.
///
/// # Errors
///
/// [`BundleError`] for any bundle that is too large, of the wrong arity or domain at any
/// layer, over a bound, truncated, carrying trailing bytes, not canonical in an item, or
/// out of order. Nothing about the input is echoed.
pub fn decode_signed(bytes: &[u8]) -> Result<SignedBundle, BundleError> {
    if bytes.len() > MAX_BUNDLE_LEN {
        return Err(BundleError::TooLarge);
    }
    decode_shared(Arc::from(bytes))
}

/// [`decode_signed`] over bytes already shared, which the bundle then keeps without a
/// copy: the restart path, which holds the custody's bytes (bn-3snfi).
///
/// # Errors
///
/// As [`decode_signed`].
pub fn decode_signed_shared(bytes: Arc<[u8]>) -> Result<SignedBundle, BundleError> {
    decode_shared(bytes)
}

fn decode_shared(content: Arc<[u8]>) -> Result<SignedBundle, BundleError> {
    let bytes: &[u8] = &content;
    if bytes.len() > MAX_BUNDLE_LEN {
        return Err(BundleError::TooLarge);
    }
    let [body, signature] =
        <[&[u8]; 2]>::try_from(unframe_exact(bytes, 2, MAX_BUNDLE_LEN, "signed bundle")?)
            .map_err(|_| BundleError::Frames("signed bundle"))?;
    if signature.len() > MAX_SIGNATURE_RECORD_LEN {
        return Err(BundleError::Entry("signature"));
    }
    let decoded = BundleBody::decode(body)?;
    // `frames(2)` is a count, the body's length, the body, the signature's length, and the
    // signature, with no trailing byte (`unframe_exact` checked all of it).
    let body_range = 8..8 + body.len();
    let signature_range = body_range.end + 4..bytes.len();
    if bytes[body_range.clone()] != *body || bytes[signature_range.clone()] != *signature {
        return Err(BundleError::Frames("signed bundle"));
    }
    Ok(SignedBundle {
        content,
        body_bytes: body_range,
        body: decoded,
        signature: signature_range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_count_beyond_the_bytes_left_is_refused_before_allocation() {
        // Count claims u32::MAX items in four bytes: refused on the count, not by running
        // out of input after reserving space for them.
        let mut bytes = u32::MAX.to_be_bytes().to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(
            unframe(&bytes, usize::MAX, 16, "t"),
            Err(BundleError::Frames("t"))
        );
        // A count within the bound but beyond the bytes left.
        let mut bytes = 3u32.to_be_bytes().to_vec();
        bytes.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(unframe(&bytes, 8, 16, "t"), Err(BundleError::Frames("t")));
    }

    #[test]
    fn frames_round_trip_and_refuse_truncation_trailing_bytes_and_long_items() {
        let items = vec![b"ab".to_vec(), Vec::new(), b"xyz".to_vec()];
        let framed = frame(&items);
        let back: Vec<Vec<u8>> = unframe(&framed, 3, 3, "t")
            .expect("well formed")
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect();
        assert_eq!(back, items);
        assert!(unframe(&framed[..framed.len() - 1], 3, 3, "t").is_err());
        let mut trailing = framed.clone();
        trailing.push(0);
        assert!(unframe(&trailing, 3, 3, "t").is_err());
        assert!(unframe(&framed, 2, 3, "t").is_err(), "count over bound");
        assert!(unframe(&framed, 3, 2, "t").is_err(), "item over bound");
        assert!(unframe(&[0, 0], 3, 3, "t").is_err(), "no count");
    }

    #[test]
    fn a_layer_of_the_wrong_arity_or_domain_is_refused_before_its_items_are_read() {
        let body = BundleBody {
            allowed: AllowedSigners::new(),
            contracts: Vec::new(),
            links: Vec::new(),
        }
        .encode();
        let good = encode_signed(&body, b"sig");
        assert!(decode_signed(&good).is_ok());
        let three = frame(&[body.clone(), b"sig".to_vec(), Vec::new()]);
        assert_eq!(
            decode_signed(&three),
            Err(BundleError::Frames("signed bundle"))
        );
        let wrong_domain = frame(&[
            b"continuum.intent-bundle.v0".to_vec(),
            frame(&[]),
            frame(&[]),
            frame(&[]),
        ]);
        assert_eq!(
            decode_signed(&encode_signed(&wrong_domain, b"sig")),
            Err(BundleError::Shape("domain"))
        );
        let long_signature = encode_signed(&body, &vec![0; MAX_SIGNATURE_RECORD_LEN + 1]);
        assert_eq!(
            decode_signed(&long_signature),
            Err(BundleError::Entry("signature"))
        );
    }

    #[test]
    fn an_oversize_bundle_is_refused_before_decoding() {
        let bytes = vec![0u8; MAX_BUNDLE_LEN + 1];
        assert_eq!(decode_signed(&bytes), Err(BundleError::TooLarge));
    }
}
