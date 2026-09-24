//! The semantic journal: an append-only sequence of events, one canonical encoding, and
//! one digest.
//!
//! # The encoding
//!
//! ```text
//! journal := MAGIC  u32(ENCODING_VERSION)  u64(count)  event*
//! event   := u64(seq)  u8(family tag)  u32(len)  payload[len]
//! ```
//!
//! `seq` is the event's position, dense from zero, so it is redundant by design: a decoder
//! that finds any other number has found a second spelling and refuses it. The payload
//! is the family's own encoding (`src/family/*.rs`), framed by its length so that a
//! decoder can say *which* event is malformed rather than losing the stream.
//!
//! The encoding carries no timestamp, no address, no actor identity and no substrate
//! handle. Two runs whose semantic events are equal are byte-equal, whatever order the
//! scheduler visited the actors in to get there.
//!
//! # Identity
//!
//! [`Journal::digest`] is BLAKE3 over the canonical encoding, through `continuum-value`'s
//! ADR-0013 seam. It is an index, not an identity: two journals are the same journal
//! when their encodings are byte-equal, and [`Journal`]'s own `PartialEq` is that
//! relation.

use continuum_value::identity::{Blake3Hasher, ContentHasher, Digest256};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::{EventBody, Family};

/// The fixed header every journal encoding starts with.
pub const MAGIC: &[u8] = b"continuum/semantic-journal\n";

/// The encoding version this crate writes.
///
/// An additive family (a sibling PR-14 bullet filling its own file) does not bump it:
/// earlier bytes still decode to the same journal. A change to the framing or to an
/// existing family's payload does. The version is an encoding contract in the sense of
/// `schemas/README.md`'s `schema_epoch`, not one of ADR-0018's six epochs: it pins no
/// semantics, but its advance obeys ADR-0018's rules (a compatibility statement, at
/// most two versions read, an unknown version refused).
///
/// Version 2 (bn-36wy3, cr-3pu5cu) adds tags to three existing families: lifecycle
/// task steps 6 (`cancel`) and 7 (`cancel-requested`), cancel cause 3 (`deadline`),
/// and virtual time event 5 (`deadline`). Compatibility statement: a version-1 journal
/// was `Preserved`. It decoded to the same journal and lifted to the same verdict; its
/// canonical re-encoding was version 2, a new identity.
///
/// Version 3 (bn-20d8u, RFC 0026 correction 58) records a fail-stop crash: lifecycle
/// event 8 (`region-crashed`), effect event 4, obligation event 6 and time event 6
/// (each `fenced`), and it changes an existing payload: obligation event 5
/// (`region-settled`) gains a third set, the obligations fenced in the region.
/// Compatibility statement: a version-2 journal is `Preserved`. It is read under the
/// version-2 grammar, its settles have an empty fenced set, and it decodes to the same
/// journal and lifts to the same verdict; its canonical re-encoding is version 3, a new
/// identity. Under ADR-0018's two-version rule, version 1 is no longer read
/// ([`DecodeError::UnsupportedVersion`]). Nothing is rewritten in place: no persisted
/// or published artifact carries these bytes (the journal is produced and judged in
/// process), and the journal is not the protocol's IDL, so no protocol version moves.
///
/// [`DecodeError::UnsupportedVersion`]: crate::encoding::DecodeError::UnsupportedVersion
pub const ENCODING_VERSION: u32 = 3;

/// Every encoding version this crate reads: the current one and its predecessor
/// (ADR-0018: at most two held at once). A version-2 journal is read under the
/// version-2 grammar, so a tag added in version 3 is
/// [`DecodeError::TagNotInVersion`] there.
pub const READ_VERSIONS: [u32; 2] = [2, 3];

/// One semantic event: its position and its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticEvent {
    seq: u64,
    body: EventBody,
}

impl SemanticEvent {
    /// Position in the journal, dense from zero.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The payload.
    #[must_use]
    pub const fn body(&self) -> &EventBody {
        &self.body
    }

    /// The family.
    #[must_use]
    pub const fn family(&self) -> Family {
        self.body.family()
    }

    /// A canonical one-line rendering: `seq family event`.
    #[must_use]
    pub fn render(&self) -> String {
        format!("{} {}", self.seq, self.body.render())
    }
}

/// An append-only sequence of semantic events.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Journal {
    events: Vec<SemanticEvent>,
}

impl Journal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `body` as the next event, returning its sequence number, or `None` when a
    /// `u64` cannot number it.
    pub fn append(&mut self, body: EventBody) -> Option<u64> {
        let seq = u64::try_from(self.events.len()).ok()?;
        self.events.push(SemanticEvent { seq, body });
        Some(seq)
    }

    /// The events, in order.
    #[must_use]
    pub fn events(&self) -> &[SemanticEvent] {
        &self.events
    }

    /// How many events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the journal is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The canonical encoding.
    ///
    /// # Errors
    ///
    /// [`EncodeError`] only for a field or journal too large for its length prefix.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut out = Encoder::new();
        out.raw(MAGIC);
        out.u32(ENCODING_VERSION);
        out.u64(u64::try_from(self.events.len()).map_err(|_| EncodeError::TooManyEvents)?);
        for event in &self.events {
            out.u64(event.seq);
            out.tag(event.family().tag());
            let mut payload = Encoder::new();
            event.body.encode(&mut payload)?;
            out.bytes(&payload.finish())?;
        }
        Ok(out.finish())
    }

    /// The journal these canonical bytes encode.
    ///
    /// # Errors
    ///
    /// A [`DecodeError`] naming where the bytes stop being canonical: anything
    /// [`Self::encode`] would not have written is refused.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut input = Decoder::new(bytes);
        input.expect_raw(MAGIC)?;
        let version_at = input.offset();
        let version = input.u32()?;
        if !READ_VERSIONS.contains(&version) {
            return Err(DecodeError::UnsupportedVersion {
                version,
                at: version_at,
            });
        }
        let count = input.u64()?;
        let mut journal = Self::new();
        for expected in 0..count {
            let seq = input.u64()?;
            if seq != expected {
                return Err(DecodeError::SequenceGap {
                    expected,
                    found: seq,
                });
            }
            let tag_at = input.offset();
            let tag = input.tag()?;
            let family = Family::from_tag(tag).ok_or(DecodeError::UnknownTag {
                table: "family",
                tag,
                at: tag_at,
            })?;
            let payload = input.bytes()?;
            let mut inner = Decoder::with_version(payload, version);
            let body = EventBody::decode(family, &mut inner, seq)?;
            if !inner.is_exhausted() {
                return Err(DecodeError::PayloadLength { seq });
            }
            journal.events.push(SemanticEvent { seq, body });
        }
        if !input.is_exhausted() {
            return Err(DecodeError::TrailingBytes { at: input.offset() });
        }
        Ok(journal)
    }

    /// BLAKE3 over the canonical encoding.
    ///
    /// # Errors
    ///
    /// As for [`Self::encode`].
    pub fn digest(&self) -> Result<Digest256, EncodeError> {
        Ok(Blake3Hasher::hash(&self.encode()?))
    }

    /// A canonical text rendering, one event per line. For people and diffs; the bytes
    /// of [`Self::encode`] are the artifact.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for event in &self.events {
            out.push_str(&event.render());
            out.push('\n');
        }
        out
    }
}
