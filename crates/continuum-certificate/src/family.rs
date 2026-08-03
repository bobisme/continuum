//! The routing table: which trusted checker owns a certificate's bytes.
//!
//! # Why the magic, and only the magic
//!
//! Each of the four `continuum-kernel-*` crates opens its wire form with a distinct
//! eight-byte magic — `CONTCERT`, `CONTSATC`, `CONTSMTC`, `CONTTMPC` — and each rejects
//! any other. That is already a total, disjoint partition of the artifacts the trusted
//! checking base understands, so routing needs no registry of its own and no second
//! notion of "family" to drift from theirs.
//!
//! The constants below are therefore *taken from* the kernels rather than restated:
//! [`Family::magic`] returns `continuum_kernel_core::wire::MAGIC` and its three
//! siblings. A kernel that changed its magic would change this table in the same
//! commit, because there is only one table.
//!
//! Routing reads at most eight bytes and consumes none of them. The whole artifact,
//! magic included, is what [`crate::check_certificate`] hands to the kernel, whose own
//! decoder re-reads it from byte zero and re-validates the magic it was routed by. The
//! read here is a dispatch hint; the kernel remains the only authority over the bytes.

/// The length of the family magic every certificate in the trusted checking base
/// begins with.
///
/// Not an independent constant: `tests/family_routing.rs` asserts it equals the length
/// of all four kernels' own `MAGIC` arrays.
pub const MAGIC_BYTES: usize = 8;

/// Which crate of the trusted checking base owns a certificate's wire contract.
///
/// One variant per `continuum-kernel-*` crate. There is no "other" variant and no
/// catch-all: a byte string that names no family is a [`RoutingFault`], which is a
/// different kind of fact from anything a checker says about a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// `CONTCERT` — `continuum-kernel-core`: finite-closure and state-type
    /// certificates.
    Core,
    /// `CONTSATC` — `continuum-kernel-sat`: LRAT refutations.
    Sat,
    /// `CONTSMTC` — `continuum-kernel-smt`: Alethe-shaped SMT refutations.
    Smt,
    /// `CONTTMPC` — `continuum-kernel-temporal`: ranking and fair-SCC-exclusion
    /// witnesses.
    Temporal,
}

impl Family {
    /// Every family, in a fixed order.
    ///
    /// The order is this enum's declaration order and is the order
    /// [`Self::from_magic`] searches, so routing is a pure function of the bytes on
    /// every platform (INV-005).
    pub const ALL: [Self; 4] = [Self::Core, Self::Sat, Self::Smt, Self::Temporal];

    /// The eight bytes this family's certificates begin with.
    ///
    /// Read straight out of the owning kernel's `wire` module; this crate declares no
    /// magic of its own.
    #[must_use]
    pub const fn magic(self) -> [u8; MAGIC_BYTES] {
        match self {
            Self::Core => continuum_kernel_core::wire::MAGIC,
            Self::Sat => continuum_kernel_sat::wire::MAGIC,
            Self::Smt => continuum_kernel_smt::wire::MAGIC,
            Self::Temporal => continuum_kernel_temporal::wire::MAGIC,
        }
    }

    /// The name of the crate that checks this family.
    ///
    /// For diagnostics and for receipts' `checker` field; the identity a receipt
    /// records is the kernel's own qualified wire epoch, which only that kernel can
    /// state (INV-014).
    #[must_use]
    pub const fn checker_crate(self) -> &'static str {
        match self {
            Self::Core => "continuum-kernel-core",
            Self::Sat => "continuum-kernel-sat",
            Self::Smt => "continuum-kernel-smt",
            Self::Temporal => "continuum-kernel-temporal",
        }
    }

    /// The family a magic names, or `None` when no kernel claims it.
    #[must_use]
    pub fn from_magic(magic: &[u8; MAGIC_BYTES]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|family| family.magic() == *magic)
    }

    /// Route a byte string to the checker that owns it, without checking it.
    ///
    /// Reads the leading [`MAGIC_BYTES`] and nothing else. Exposed because dispatch is
    /// a useful answer on its own — logging, storage, deciding which kernel's receipt
    /// vocabulary applies — and because a caller that wants only the family should not
    /// have to run a checker to learn it.
    ///
    /// # Errors
    ///
    /// [`RoutingFault::TooShortForMagic`] when the input cannot carry a magic at all;
    /// [`RoutingFault::UnknownFamily`] when it carries one no kernel claims.
    pub fn route(bytes: &[u8]) -> Result<Self, RoutingFault> {
        let Some(magic) = bytes
            .get(..MAGIC_BYTES)
            .and_then(|head| <[u8; MAGIC_BYTES]>::try_from(head).ok())
        else {
            return Err(RoutingFault::TooShortForMagic {
                available: bytes.len(),
            });
        };
        Self::from_magic(&magic).ok_or(RoutingFault::UnknownFamily { magic })
    }
}

/// Why a byte string reached no checker.
///
/// # Why this is not a `Rejected` and not an `Unsupported`
///
/// > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and
/// > incomplete proof search are distinct outcomes.
/// >
/// > — `notes/plan/plan.md`, INV-008 "Typed inconclusiveness"
///
/// Each kernel keeps "these bytes are not a certificate of my family" (`Rejected`) apart
/// from "these bytes name a contract I do not implement" (`Unsupported`), and it can
/// only tell them apart because it has a decoder. This crate has none, by design: an
/// unrecognised magic could be a corrupted `CONTCERT`, a certificate family a later
/// kernel will own, or a JPEG. Answering `Rejected` would call a possibly-valid
/// artifact invalid; answering `Unsupported` would call a possibly-corrupt one
/// merely-unimplemented. So the answer is neither — it is the honest third fact, that
/// nothing checked these bytes and no checker was in a position to say why.
///
/// A caller that needs the distinction has the material to get it: [`Family::ALL`]
/// names every magic this build routes, and a byte string bearing one of them is
/// answered by that kernel's own vocabulary through
/// [`Outcome::Checked`](crate::verdict::Outcome::Checked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingFault {
    /// Fewer than [`MAGIC_BYTES`] bytes: the artifact cannot carry a family magic at
    /// all, whatever else it is.
    TooShortForMagic {
        /// How many bytes the input actually had.
        available: usize,
    },
    /// Eight readable bytes naming no family in the trusted checking base.
    UnknownFamily {
        /// The magic as read, so a diagnostic can print what arrived rather than what
        /// was expected.
        magic: [u8; MAGIC_BYTES],
    },
}
