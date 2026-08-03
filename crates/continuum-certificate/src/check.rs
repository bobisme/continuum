//! The one entry point: wire bytes to a kernel's verdict (plan §20).
//!
//! > the `continuum-kernel-*` crates form the trusted checking base: … and a
//! > serialization boundary between every engine and the kernel — a certificate is
//! > checked from its wire form, never from shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! # What happens to the bytes
//!
//! Nothing. [`check_certificate`] does not copy, trim, reframe, or re-length the input:
//! it reads the leading magic to pick a checker and then passes the *same slice*,
//! magic included, to that checker's own `check_certificate`. The kernel's decoder
//! starts at byte zero and re-validates the magic it was routed by, so the artifact is
//! read exactly once, by the only crate entitled to read it. A dispatcher that stripped
//! a header would have become a second parser of the format — the thing docs/03 §8's
//! independence argument exists to prevent.
//!
//! # Determinism
//!
//! Routing reads eight bytes, compares them against four constants in a fixed order,
//! and calls one function. No clock, no randomness, no socket, no allocation, no
//! iteration order that is not this crate's own declaration order (INV-005; ADR-0003;
//! docs/12 §1). Two runs over the same bytes produce identical outcomes on any
//! platform, and any nondeterminism a caller observes came from the kernel, which is
//! covenant-bound not to have any.

use crate::family::Family;
use crate::verdict::{KernelVerdict, Outcome};

/// Check one certificate from its wire form, through whichever kernel owns its family.
///
/// The crate's whole public checking surface. It takes bytes — never a structure a
/// producer could hand over — and returns the routed kernel's own verdict verbatim,
/// or the fact that no kernel owns the bytes.
///
/// It cannot panic on any input: routing is a bounded read of at most eight bytes
/// through a fallible conversion, and each kernel's own `check_certificate` is total
/// by covenant (docs/12 §11; the kernels' `garbage_bytes_never_panic` suites).
#[must_use]
pub fn check_certificate(bytes: &[u8]) -> Outcome {
    match Family::route(bytes) {
        Err(fault) => Outcome::Unroutable(fault),
        Ok(Family::Core) => Outcome::Checked(KernelVerdict::Core(
            continuum_kernel_core::check_certificate(bytes),
        )),
        Ok(Family::Sat) => Outcome::Checked(KernelVerdict::Sat(
            continuum_kernel_sat::check_certificate(bytes),
        )),
        Ok(Family::Smt) => Outcome::Checked(KernelVerdict::Smt(
            continuum_kernel_smt::check_certificate(bytes),
        )),
        Ok(Family::Temporal) => Outcome::Checked(KernelVerdict::Temporal(
            continuum_kernel_temporal::check_certificate(bytes),
        )),
    }
}
