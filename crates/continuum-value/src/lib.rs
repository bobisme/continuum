//! `continuum-value` — exact finite values, canonical encoding, and content identity
//! (plan §4.4, PR 2).
//!
//! # Responsibility
//!
//! Exact finite values carried over from Revision 2, their canonical encoding, and
//! their total order.
//!
//! Content identity per ADR-0013: in certified lanes canonical content identity is
//! primary and hash collisions are resolved by canonical comparison; 256-bit-hash
//! identity is permitted only in explicitly labeled non-certified modes. That rule is
//! implemented in [`identity`] (PR 2), directly on top of [`value`]'s canonical
//! encoding: a certified identity *is* the encoding, so it carries no hasher and no
//! hash-vendor decision can change what two values being "the same artifact" means.
//! The hash the ADR gives an indexing role sits behind the [`identity::ContentHasher`]
//! seam, currently filled by a labeled, deliberately non-cryptographic placeholder.
//!
//! Compatibility epochs live here too, in [`epoch`] (PR-1 / IMPL-02). Plan §4.6 states
//! it directly — "Epochs are content identities" — and docs/33 "Trust boundary" places
//! *content identity and authorization* inside the smallest trust base, which is this
//! crate's declared responsibility. Because this is the workspace's only declared leaf
//! crate, the kernel, the certificate formats, the evidence graph, the corpus manifests,
//! the proof client, and `continuumd` can all agree on one epoch vocabulary without any
//! of them importing another's semantics, and without pulling anything new into the
//! certificate/kernel dependency closure.
//!
//! The assurance vocabulary lives here for the same reason, in [`assurance`]
//! (PR-1 / IMPL-03). RFC 0026 makes the assurance envelope "required on every semantic
//! verdict", so the daemon, the CLI, the evidence graph, the context packs, the
//! certificates and the corpus receipts all render the same nine dimensions, the same
//! assurance ladder, and the same typed INV-008 inconclusive reasons. Plan §20 names no
//! assurance crate; putting the vocabulary anywhere else would make the trusted checking
//! base depend on a consumer of its own results, which is the edge §20's dependency rules
//! exist to prevent. In the only declared leaf crate it costs no dependency edge at all.
//!
//! Owning the *types* is not owning the *policy*: when and how each epoch advances stays
//! with the crate that owns the artifact class it versions, and which assurance level a
//! task needs, which engine produced a dimension, and when a claim may be promoted stay
//! with the crates that own those decisions.
//!
//! # Dependency-boundary contract
//!
//! - The canonical value decoder is a trust-base component (docs/33): no async, no
//!   search, no adapter, and no Forge dependency.
//! - Leaf crate — it may not import any other Continuum crate.
//!
//! The exact finite values, their canonical encoding, and their total order live in
//! [`value`] (PR 2) and ADR-0013 content identity in [`identity`] (PR 2); [`epoch`]
//! landed with PR-1 / IMPL-02 and [`assurance`] with PR-1 / IMPL-03.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod assurance;
pub mod epoch;
pub mod identity;
pub mod value;
