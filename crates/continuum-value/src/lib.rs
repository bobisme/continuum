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
//! identity is permitted only in explicitly labeled non-certified modes.
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
//! Owning the *types* is not owning the *policy*: when and how each epoch advances stays
//! with the crate that owns the artifact class it versions.
//!
//! # Dependency-boundary contract
//!
//! - The canonical value decoder is a trust-base component (docs/33): no async, no
//!   search, no adapter, and no Forge dependency.
//! - Leaf crate — it may not import any other Continuum crate.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The exact finite values and their canonical encoding land in PR 2;
//! [`epoch`] landed with PR-1 / IMPL-02.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod epoch;
