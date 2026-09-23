//! Canonical names for variables, actions, and predicates.
//!
//! Decision: RFC 0005 (the certificate wire form, whose token grammar and byte order this
//! type mirrors) and RFC 0003 (model names).
//!
//! # Why a newtype, and why *this* order
//!
//! A model's variable positions, its action indices, and the order of its state
//! vectors are all decided by comparing names. The comparison is therefore a
//! semantic decision, not a convenience, and it has to be the same one the kernel
//! makes when it decodes the certificate the engine will eventually emit:
//!
//! > Ordered by bytes, which is the order the canonicity rules are stated in.
//! >
//! > — `crates/continuum-kernel-core/src/wire.rs:312` (`wire::Token`)
//!
//! [`Ident`] therefore derives [`Ord`] on a [`String`], which is byte-lexicographic,
//! and the grammar below is exactly the kernel's token grammar: non-empty printable
//! ASCII, at most [`MAX_IDENT_BYTES`] bytes
//! (`crates/continuum-kernel-core/src/wire.rs:311`, `MAX_TOKEN_BYTES`). A name this
//! module admits is a name the kernel's decoder admits, and two names this module
//! orders are two names the kernel orders the same way.
//!
//! # Why not `continuum_value::value::Name`
//!
//! `continuum-value` owns a canonical-identifier type with the *same grammar* —
//! "non-empty printable ASCII" (`crates/continuum-value/src/value.rs:283-287`) — and
//! reusing it would have been the obvious move. Its [`Ord`] is a different relation:
//!
//! ```text
//! fn compare_blobs(left: &[u8], right: &[u8]) -> Ordering {
//!     left.len().cmp(&right.len()).then_with(|| left.cmp(right))
//! }
//! ```
//!
//! — `crates/continuum-value/src/value.rs:1083-1085`
//!
//! That is shortlex: length first, bytes second. It is right for `continuum-value`,
//! whose canonical encoding is length-prefixed, and wrong here. On the Die Hard
//! corpus model the two relations disagree outright — byte order gives
//! `BigToSmall < EmptyBig < EmptySmall < FillBig < FillSmall < SmallToBig`, shortlex
//! gives `FillBig < EmptyBig < FillSmall < EmptySmall < BigToSmall < SmallToBig` —
//! so a model keyed by `Name` would number its actions in an order the kernel's
//! decoder rejects as not strictly ascending. Borrowing the type and overriding its
//! order would leave two orders on one type, which is the ambiguity docs/12 §1
//! (`GOV-1-12`, "ambiguous behavior is an error") exists to forbid. The engine keeps
//! its own name type instead, and this crate depends on no workspace crate at all.

use core::fmt;

/// The longest admissible name, in bytes.
///
/// `crates/continuum-kernel-core/src/wire.rs:119` fixes `MAX_TOKEN_BYTES = 128` for
/// every digest, variable name, and action name on the certificate wire. A model that
/// declared a longer name could not be certified, so it is refused at declaration
/// time rather than at emission time.
pub const MAX_IDENT_BYTES: usize = 128;

/// A canonical name: non-empty printable ASCII, ordered by bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ident(String);

impl Ident {
    /// Build a name from its canonical spelling.
    ///
    /// # Errors
    ///
    /// [`IdentError::Empty`] for the empty string, [`IdentError::TooLong`] beyond
    /// [`MAX_IDENT_BYTES`], and [`IdentError::NotPrintableAscii`] for the first byte
    /// outside `0x21..=0x7E`.
    pub fn new(name: &str) -> Result<Self, IdentError> {
        if name.is_empty() {
            return Err(IdentError::Empty);
        }
        if name.len() > MAX_IDENT_BYTES {
            return Err(IdentError::TooLong {
                bytes: name.len(),
                max: MAX_IDENT_BYTES,
            });
        }
        for (index, byte) in name.bytes().enumerate() {
            if !byte.is_ascii_graphic() {
                return Err(IdentError::NotPrintableAscii { index, byte });
            }
        }
        Ok(Self(name.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a string is not a canonical name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentError {
    /// The empty string.
    Empty,
    /// Longer than [`MAX_IDENT_BYTES`], so the kernel's token decoder would refuse it.
    TooLong {
        /// The length that was offered.
        bytes: usize,
        /// [`MAX_IDENT_BYTES`].
        max: usize,
    },
    /// A byte outside printable ASCII, which would admit two spellings of one name.
    NotPrintableAscii {
        /// Byte offset of the first offending byte.
        index: usize,
        /// The offending byte.
        byte: u8,
    },
}

impl fmt::Display for IdentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a name may not be empty"),
            Self::TooLong { bytes, max } => {
                write!(f, "name is {bytes} bytes; the limit is {max}")
            }
            Self::NotPrintableAscii { index, byte } => write!(
                f,
                "byte {byte:#04x} at offset {index} is not printable ASCII"
            ),
        }
    }
}

impl core::error::Error for IdentError {}
