//! The ADR-0013 identity discipline, written once for the whole crate.
//!
//! # Why this module exists
//!
//! Every field group in this crate carries an identity, and all nine of them are the
//! *same* discipline:
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`, "Decision"
//!
//! Nine independently written copies of that discipline is nine places a future edit
//! could quietly make one group's identity a digest, or make its equality consult a
//! hash — in a crate whose whole job is that a *weakening* cannot hide. So the
//! discipline is [`CanonicalIdentity`], stated here and nowhere else: the identity
//! *is* the canonical preimage bytes, not a struct holding a digest, so equality is
//! canonical comparison and [`CanonicalIdentity::digest`] can only ever index.
//!
//! The eight PR-4 field-group bones landed concurrently, each in its own workspace
//! copy of this crate, and a shared module is a shared line; the copies were the
//! right call then and [`crate::observers::ObserverIdentity`]'s doc comment recorded
//! the seam ("when a later bone gives the crate one shared identity newtype, this
//! type and its siblings […] collapse into it"). This is that bone.
//!
//! # Why the nine public names stay
//!
//! [`crate::property::PropertyIdentity`], [`crate::observers::ObserverIdentity`], and
//! their seven siblings are the return types of nine public `identity()` accessors.
//! Collapsing them into one public `CanonicalIdentity` would let a caller pass an
//! observer's identity where a claim's is expected and have it compile — the type
//! system stops asking "identity of what?" exactly where the contract cares most. So
//! [`canonical_identity!`] declares each group's identity as a distinct newtype over
//! the one shared core: one discipline, nine types a checker cannot confuse.
//!
//! The generated [`core::fmt::Debug`] is hand-written rather than derived so that a
//! newtype renders exactly as the nine hand-written structs did — `PropertyIdentity {
//! canonical: [123, …] }`, not `PropertyIdentity(CanonicalIdentity { … })`. Nothing
//! in the crate reads that rendering, but this refactor's bar is that nothing
//! *could* tell.

use core::fmt;

use continuum_value::identity::{ContentHasher, Digest256};

/// Canonical bytes that are an identity.
///
/// This type *is* the identity-preimage bytes, exactly as
/// `continuum_value::identity::ContentIdentity` is the CVNF-1 bytes: not a struct
/// holding a digest, not a newtype over a hash. There is no hasher parameter and no
/// field a hash could reach, so two artifacts are the same artifact precisely when
/// their canonical encodings agree — a fact no hash-vendor decision can change.
///
/// The bytes are UTF-8 by construction (RFC 0037 ID5), so [`fmt::Display`] renders
/// the identity as the canonical JSON itself. An identity that can be read is an
/// identity a reviewer can check by hand, which is the point of ID7's checkable list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CanonicalIdentity {
    canonical: Vec<u8>,
}

impl CanonicalIdentity {
    /// The identity of an already-canonical byte string.
    ///
    /// `pub(crate)` and reached only through a field group's own private
    /// `of_bytes`, on purpose: an identity is derived from a value, never asserted
    /// about one. Compare `ContentIdentity::of`.
    pub(crate) const fn of_bytes(canonical: Vec<u8>) -> Self {
        Self { canonical }
    }

    /// The canonical bytes this identity is.
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// A digest of the canonical bytes, for indexing only.
    ///
    /// ADR-0013's second clause: "Hashes index and partition; collisions resolve by
    /// exact comparison." Nothing in this type consults the digest, so a caller that
    /// keys a map by it must still compare [`canonical_bytes`](Self::canonical_bytes)
    /// on a hit — which is what `continuum_value::identity::IdentityIndex` does, and
    /// why the hasher is a parameter rather than a decision made here.
    pub(crate) fn digest<H: ContentHasher>(&self) -> Digest256 {
        H::hash(&self.canonical)
    }
}

impl fmt::Display for CanonicalIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match core::str::from_utf8(&self.canonical) {
            Ok(text) => f.write_str(text),
            Err(_) => Err(fmt::Error),
        }
    }
}

/// Declare one field group's public identity newtype over [`CanonicalIdentity`].
///
/// The caller supplies the doc comment — each group's identity has its own thing to
/// say about *which* preimage it is over — and nothing else. The discipline itself
/// (bytes are the identity, a digest only indexes, `Display` is the canonical JSON)
/// comes from [`CanonicalIdentity`] and is not restated per group.
macro_rules! canonical_identity {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($crate::identity::CanonicalIdentity);

        impl $name {
            /// The identity of an already-canonical byte string.
            ///
            /// Private on purpose: an identity is derived from a value, never
            /// asserted about one.
            fn of_bytes(canonical: ::std::vec::Vec<u8>) -> Self {
                Self($crate::identity::CanonicalIdentity::of_bytes(canonical))
            }

            /// The canonical bytes this identity is.
            #[must_use]
            pub fn canonical_bytes(&self) -> &[u8] {
                self.0.canonical_bytes()
            }

            /// A digest of the canonical bytes, for indexing only (ADR-0013).
            ///
            /// The hasher is a parameter, so this crate makes no hash-vendor
            /// decision; a caller that keys a map by the digest must still compare
            /// [`canonical_bytes`](Self::canonical_bytes) on a hit.
            #[must_use]
            pub fn digest<H: ::continuum_value::identity::ContentHasher>(
                &self,
            ) -> ::continuum_value::identity::Digest256 {
                self.0.digest::<H>()
            }
        }

        // Hand-written so the rendering is the one the nine hand-written structs
        // produced, byte for byte: `Name { canonical: [..] }`.
        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(::core::stringify!($name))
                    .field("canonical", &self.0.canonical_bytes())
                    .finish()
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

pub(crate) use canonical_identity;
