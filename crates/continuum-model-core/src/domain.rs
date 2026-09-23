//! Declared state domains: a named variable and the inclusive integer range it
//! ranges over.
//!
//! Decision: RFC 0005 (the finite-closure certificate wire form, whose `variable := name lo hi`
//! this type mirrors) and RFC 0003 (model state domains).
//!
//! # Why inclusive `i64` ranges and nothing else
//!
//! > `domain       := variable_count:u16 variable*`
//! > `variable     := name:token lo:i64 hi:i64`
//! >
//! > — `crates/continuum-kernel-core/src/wire.rs:57-58`
//!
//! The certificate this engine must eventually emit carries a state as a vector of
//! `i64` and a variable as a name with two `i64` bounds. A model whose variables
//! ranged over anything else could not be certified, so the model layer declares
//! exactly what the wire form can carry. `continuum_value::value::Value`'s richer
//! lattice — sets, records, bitvectors, opaque domains — is the right vocabulary for
//! the CIR and the wrong one here: it has no total injection into `i64`, and choosing
//! one (`Nat(3)` and `Int(3)` "are distinct values with distinct encodings",
//! `crates/continuum-value/src/value.rs:618-631`) would put a coercion decision
//! between the model and the certificate that neither the plan nor the wire form
//! authorises. That crate is not a dependency of this one; see the crate root.

use core::fmt;

use crate::ident::Ident;

/// An inclusive integer range: every `v` with `lo <= v <= hi`.
///
/// Empty ranges are unrepresentable. The kernel refuses `lo > hi` outright
/// (`Rejection::InvertedVariableRange`, `crates/continuum-kernel-core/src/wire.rs:531-533`),
/// and a variable with no admissible value makes the whole state space empty while
/// looking like an ordinary declaration, so it is rejected at declaration time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Domain {
    lo: i64,
    hi: i64,
}

impl Domain {
    /// Declare the inclusive range `lo..=hi`.
    ///
    /// # Errors
    ///
    /// [`DomainError::Empty`] when `lo > hi`.
    pub const fn new(lo: i64, hi: i64) -> Result<Self, DomainError> {
        if lo > hi {
            return Err(DomainError::Empty { lo, hi });
        }
        Ok(Self { lo, hi })
    }

    /// The smallest admissible value.
    #[must_use]
    pub const fn lo(&self) -> i64 {
        self.lo
    }

    /// The largest admissible value.
    #[must_use]
    pub const fn hi(&self) -> i64 {
        self.hi
    }

    /// Whether `value` lies in the declared range.
    ///
    /// Deliberately identical to `wire::Variable::admits`
    /// (`crates/continuum-kernel-core/src/wire.rs:485-487`): the producer's notion of
    /// "in domain" and the checker's must not be able to disagree.
    #[must_use]
    pub const fn admits(&self, value: i64) -> bool {
        self.lo <= value && value <= self.hi
    }

    /// How many values the range admits.
    ///
    /// Computed in `i128` because `hi - lo + 1` overflows `i64` for wide ranges; the
    /// result is exact for every representable [`Domain`]. This is the per-variable
    /// factor of the state-space bound the exploration bone needs before it starts.
    #[must_use]
    pub const fn cardinality(&self) -> u128 {
        let span = (self.hi as i128).saturating_sub(self.lo as i128);
        (span as u128).saturating_add(1)
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..={}", self.lo, self.hi)
    }
}

/// Why a declared range is not a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// `lo > hi`: the range admits no value.
    Empty {
        /// The declared lower bound.
        lo: i64,
        /// The declared upper bound.
        hi: i64,
    },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty { lo, hi } => {
                write!(
                    f,
                    "range {lo}..={hi} is empty; a domain admits at least one value"
                )
            }
        }
    }
}

impl core::error::Error for DomainError {}

/// A declared state variable: a canonical name and the range it ranges over.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Variable {
    name: Ident,
    domain: Domain,
}

impl Variable {
    /// Pair a name with a domain.
    #[must_use]
    pub const fn new(name: Ident, domain: Domain) -> Self {
        Self { name, domain }
    }

    /// The variable's canonical name.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// The variable's declared domain.
    #[must_use]
    pub const fn domain(&self) -> &Domain {
        &self.domain
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in {}", self.name, self.domain)
    }
}
