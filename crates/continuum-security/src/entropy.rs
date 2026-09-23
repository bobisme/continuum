//! The operating-system entropy source behind `continuum-evidence`'s `KeyEntropy`
//! capability (plan §18.6, ADR-0054, bn-1hape).
//!
//! `continuum-evidence` reads no entropy of its own (INV-005, ADR-0003): a key is a function
//! of the 32-byte seed its `KeyEntropy` capability returns. This module is the production
//! capability, and it lives here because `continuum-security` is a boundary crate
//! (`tools/governance/check_code_policy.py` `BOUNDARY`): owning an ambient resource behind
//! an explicit capability is its job. No semantic-core crate depends on this crate, and
//! `tests/entropy_isolation.rs` checks that from the real manifests, so no deterministic
//! path can reach [`OsEntropy`].
//!
//! The source is the kernel's CSPRNG through `/dev/urandom`, read with `std` alone, so the
//! capability adds no dependency. A read failure is [`EntropyUnavailable`], and minting then
//! fails; there is no fallback to a weaker source. A 32-byte all-zero read is refused as a
//! failed source rather than accepted as a key.

use std::fs::File;
use std::io::Read;

use continuum_evidence::signing::{EntropyUnavailable, KeyEntropy, SEED_LEN, Zeroizing};

/// The path read. The Linux and BSD kernels expose their CSPRNG here.
pub const OS_ENTROPY_PATH: &str = "/dev/urandom";

/// Operating-system entropy, as a `KeyEntropy` capability.
///
/// Constructed only by an explicit call ([`OsEntropy::new`]) and handed to minting as a
/// `&mut dyn KeyEntropy`; nothing reaches it ambiently.
#[derive(Debug, Default)]
pub struct OsEntropy {
    _explicit: (),
}

impl OsEntropy {
    /// The operating-system entropy capability.
    #[must_use]
    pub const fn new() -> Self {
        Self { _explicit: () }
    }
}

impl KeyEntropy for OsEntropy {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        let mut source = File::open(OS_ENTROPY_PATH).map_err(|_| EntropyUnavailable)?;
        let mut seed = Zeroizing::new([0u8; SEED_LEN]);
        source
            .read_exact(seed.as_mut())
            .map_err(|_| EntropyUnavailable)?;
        if seed.iter().all(|byte| *byte == 0) {
            return Err(EntropyUnavailable);
        }
        Ok(seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_entropy_supplies_distinct_nonzero_seeds() {
        let mut entropy = OsEntropy::new();
        let a = entropy.seed().expect("the host exposes a CSPRNG");
        let b = entropy.seed().expect("the host exposes a CSPRNG");
        assert_ne!(*a, [0u8; SEED_LEN]);
        assert_ne!(*a, *b, "two 256-bit draws collided");
    }
}
