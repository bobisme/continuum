//! The seeded generator's only source of choice: SplitMix64, written here.
//!
//! No crates.io dependency is taken for it (`tools/governance/dependency-audits.toml`
//! requires a human audit per package, and GOV-1-04 bans the usual generator crates from
//! the core). The seed arrives as an explicit argument, which is what INV-005 and
//! ADR-0003 ask of any entropy: the stream is a pure function of the seed, so a
//! generated system reproduces from its seed alone.
//!
//! SplitMix64 is Steele, Lea and Flood's mixer ("Fast splittable pseudorandom number
//! generators", OOPSLA 2014), with the constants `java.util.SplittableRandom` uses. It
//! is not a cryptographic generator and is not asked to be one: its job is coverage of
//! a tiny shape space. Its first outputs for seed 0 are pinned by a test, so a change to
//! the stream is a visible change to every generated artifact.

/// A SplitMix64 stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// The stream for `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 bits.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ z.wrapping_shr(30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ z.wrapping_shr(27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ z.wrapping_shr(31)
    }

    /// A value in `0..bound`, or `0` when `bound` is `0`.
    ///
    /// Plain modulo reduction. The bias is below `bound / 2^64`, which for the bounds
    /// used here (a few dozen at most) no tiny generator can observe, and a rejection
    /// loop would make the number of draws per call depend on the data for no gain.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64().checked_rem(bound).unwrap_or(0)
    }

    /// A fair coin.
    pub const fn coin(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::SplitMix64;

    #[test]
    fn seed_zero_matches_the_published_reference_stream() {
        let mut stream = SplitMix64::new(0);
        assert_eq!(stream.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(stream.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(stream.next_u64(), 0x06C4_5D18_8009_454F);
    }
}
