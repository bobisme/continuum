//! The canonical byte encoding the journal is written in.
//!
//! # Why a binary framing of its own
//!
//! The journal is not a JSON-schema artifact: `cir.schema.json` is the normative shape of
//! the causal graph a journal is later *compiled into* (PR 17), and this crate does not
//! claim that shape. So the journal does not reach for `continuum_intent::canonical_json`
//! (the JSON-schema writer) or for CVNF-1 (the exact-finite-value domain). It needs one
//! property only — **one byte string per journal, and one journal per byte string** — and
//! a small prefix-free framing gives that by construction:
//!
//! - every integer is fixed-width big-endian, so no integer has two spellings;
//! - every variable-length field is a `u32` length followed by exactly that many bytes;
//! - every closed choice is a one-byte tag from a table written once, next to its type;
//! - the decoder refuses anything the encoder would not have written: an unknown tag, a
//!   short read, a length that runs past the end, and trailing bytes.
//!
//! The encoder is total and the decoder is its left inverse on everything the encoder
//! emits. `tests/pr14_impl01_journal.rs` holds both directions and a pinned golden.

use core::fmt;

/// A write-only canonical byte sink.
#[derive(Debug, Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    /// An empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One tag byte.
    pub fn tag(&mut self, tag: u8) {
        self.bytes.push(tag);
    }

    /// A `u32`, big-endian.
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    /// A `u64`, big-endian.
    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    /// Raw bytes, length-prefixed.
    ///
    /// # Errors
    ///
    /// [`EncodeError::FieldTooLong`] when the field does not fit a `u32` length.
    pub fn bytes(&mut self, field: &[u8]) -> Result<(), EncodeError> {
        let len = u32::try_from(field.len())
            .map_err(|_| EncodeError::FieldTooLong { len: field.len() })?;
        self.u32(len);
        self.bytes.extend_from_slice(field);
        Ok(())
    }

    /// A UTF-8 token, length-prefixed.
    ///
    /// # Errors
    ///
    /// [`EncodeError::FieldTooLong`] as for [`Self::bytes`].
    pub fn token(&mut self, token: &str) -> Result<(), EncodeError> {
        self.bytes(token.as_bytes())
    }

    /// Bytes written verbatim, with no length prefix. Used only for fixed headers.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// The bytes written so far.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Why a value could not be encoded.
///
/// Every field this crate writes is small, so this is reachable only through a field
/// longer than 4 GiB. It is still a typed value and never a truncation (INV-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// A length-prefixed field longer than `u32::MAX` bytes.
    FieldTooLong {
        /// The field's length.
        len: usize,
    },
    /// More events than a `u64` counts.
    TooManyEvents,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldTooLong { len } => write!(f, "a field of {len} bytes exceeds a u32 length"),
            Self::TooManyEvents => f.write_str("the journal has more events than a u64 counts"),
        }
    }
}

impl core::error::Error for EncodeError {}

/// A read cursor over canonical bytes.
#[derive(Debug)]
pub struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Decoder<'a> {
    /// A cursor at the start of `bytes`.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// The offset of the next unread byte.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.at
    }

    /// Whether every byte has been read.
    #[must_use]
    pub const fn is_exhausted(&self) -> bool {
        self.at == self.bytes.len()
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(DecodeError::Truncated { at: self.at })?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    /// One tag byte.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Truncated`] at the end of input.
    pub fn tag(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    /// A big-endian `u32`.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Truncated`] when fewer than four bytes remain.
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        let mut out = [0_u8; 4];
        out.copy_from_slice(self.take(4)?);
        Ok(u32::from_be_bytes(out))
    }

    /// A big-endian `u64`.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Truncated`] when fewer than eight bytes remain.
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        let mut out = [0_u8; 8];
        out.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(out))
    }

    /// A length-prefixed byte field.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Truncated`] when the declared length runs past the end.
    pub fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    /// A length-prefixed UTF-8 token.
    ///
    /// # Errors
    ///
    /// [`DecodeError::Truncated`], or [`DecodeError::NotUtf8`] when the field is not
    /// UTF-8.
    pub fn token(&mut self) -> Result<&'a str, DecodeError> {
        let at = self.at;
        let field = self.bytes()?;
        core::str::from_utf8(field).map_err(|_| DecodeError::NotUtf8 { at })
    }

    /// Exactly `expected`, verbatim.
    ///
    /// # Errors
    ///
    /// [`DecodeError::BadHeader`] when the bytes differ.
    pub fn expect_raw(&mut self, expected: &[u8]) -> Result<(), DecodeError> {
        let at = self.at;
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(DecodeError::BadHeader { at })
        }
    }
}

/// Why bytes are not a canonical journal.
///
/// Each arm names where the decoder stopped, so a refusal is reproducible from the bytes
/// alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended inside a field.
    Truncated {
        /// Offset of the field that ran out.
        at: usize,
    },
    /// The fixed header or encoding version is not this crate's.
    BadHeader {
        /// Offset of the header.
        at: usize,
    },
    /// A tag byte is not in its table.
    UnknownTag {
        /// Which table.
        table: &'static str,
        /// The byte read.
        tag: u8,
        /// Its offset.
        at: usize,
    },
    /// A token field is not UTF-8.
    NotUtf8 {
        /// Offset of the field.
        at: usize,
    },
    /// A token field is not a canonical reason token (`continuum_task`'s rule).
    NonCanonicalToken {
        /// Offset of the field.
        at: usize,
    },
    /// A set field is not strictly ascending, so it has a second spelling.
    UnsortedSet {
        /// Offset of the field.
        at: usize,
    },
    /// An event's sequence number is not its position. The journal numbers events
    /// densely from zero, so any other number is a second spelling.
    SequenceGap {
        /// The position.
        expected: u64,
        /// The number read.
        found: u64,
    },
    /// An event's payload length disagrees with what its family decoded.
    PayloadLength {
        /// The event's sequence number.
        seq: u64,
    },
    /// The event belongs to a family whose instrumentation has not landed, so no
    /// decoder for its payload exists. A typed absence, not a parse failure: the bytes
    /// may be well formed for a later encoding version.
    FamilyNotInstrumented {
        /// The family's stable token.
        family: &'static str,
        /// The event's sequence number.
        seq: u64,
    },
    /// Bytes remain after the last event.
    TrailingBytes {
        /// Offset of the first extra byte.
        at: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { at } => write!(f, "input ends inside the field at byte {at}"),
            Self::BadHeader { at } => write!(f, "no semantic-journal header at byte {at}"),
            Self::UnknownTag { table, tag, at } => {
                write!(f, "tag {tag} at byte {at} is not a {table}")
            }
            Self::NotUtf8 { at } => write!(f, "the token at byte {at} is not UTF-8"),
            Self::NonCanonicalToken { at } => {
                write!(f, "the token at byte {at} is not a canonical reason token")
            }
            Self::UnsortedSet { at } => {
                write!(f, "the set at byte {at} is not strictly ascending")
            }
            Self::SequenceGap { expected, found } => {
                write!(f, "event {expected} carries sequence number {found}")
            }
            Self::PayloadLength { seq } => {
                write!(f, "event {seq}'s payload length disagrees with its content")
            }
            Self::FamilyNotInstrumented { family, seq } => write!(
                f,
                "event {seq} is in family {family}, whose instrumentation has not landed"
            ),
            Self::TrailingBytes { at } => write!(f, "bytes remain after the last event at {at}"),
        }
    }
}

impl core::error::Error for DecodeError {}
