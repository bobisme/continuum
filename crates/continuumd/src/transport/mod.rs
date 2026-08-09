//! The local transport: bytes in, dispatch, bytes out.
//!
//! # What this is, and what it deliberately is not
//!
//! A pair of connected endpoints exchanging length-prefixed canonical frames, driving a
//! [`Daemon`] end to end. It is in-process, synchronous, and has no socket, no runtime,
//! and no thread — and that is not a stand-in for a real transport, it is the grain the
//! rest of the daemon is built at:
//!
//! - **`Daemon::dispatch` takes `&mut self`**, so two requests are never in flight at
//!   once. A transport that could interleave them would be describing a daemon that does
//!   not exist. `daemon`'s own documentation makes this load-bearing rather than
//!   incidental: `TaskStatus::Running` is unobservable *because* of that borrow.
//! - **INV-005 and ADR-0003 put the daemon core inside the deterministic band**, and plan
//!   §20 gives `continuumd` no async-runtime dependency edge. Adding one here to move
//!   bytes between two buffers in the same process would buy nothing and cost the
//!   property.
//! - **Nothing is session-scoped** (RFC 0026, INV-002): the connection carries a
//!   negotiated version and encoding and no semantic state, so an endpoint is a byte
//!   queue and a `Negotiated`, and reconnecting loses nothing. An evidence subscription
//!   (below) is the one thing a connection remembers, and it is a *delivery cursor* into
//!   an append-only daemon-held log rather than semantic state: dropping the connection
//!   loses the cursor and no artifact, and the frontier a fresh subscription answers with
//!   covers everything the dropped one delivered. That is `rule subscription.delivery`'s
//!   closing sentence, and it is what makes the memory admissible under INV-002 at all.
//!
//! What a unix-socket or QUIC transport adds over this is *where the bytes come from*.
//! The frame format, the codec, the dispatch, and the result encoding are all here, and
//! the byte boundary is real: [`Server::serve`] receives a `Vec<u8>` it did not build and
//! answers with a `Vec<u8>` the caller did not build.
//!
//! # The frame
//!
//! `u32` big-endian length, then that many bytes of canonical message. Fixed-width and
//! big-endian so a length has one spelling, which is the same reason the payload has one:
//! a transport whose framing admitted two encodings of "seven bytes follow" would put a
//! second spelling underneath a canonical message. [`MAX_FRAME_BYTES`] bounds a frame so
//! that a length prefix cannot ask an endpoint for an allocation before any of the
//! payload has arrived — the check RFC 0026's malformed-input discipline implies for a
//! reader that has read four bytes and nothing else.
//!
//! # The three things that cross the boundary
//!
//! | Direction | Frame | Built by |
//! |---|---|---|
//! | client → server | `ClientHello`, then `RequestEnvelope` frames | [`encode_hello`], [`encode_request`] |
//! | server → client | `ServerWelcome` **or** `ServerReject`, then `ResultEnvelope` frames, and after a subscription's response the `EvidenceEvent` frames of `rule subscription.delivery` | [`Server::open`], [`Server::serve`] |
//!
//! `ServerReject` is reachable *before* any request is served, which is the point of it:
//! "a refused connection is a typed frame rather than a transport failure" (RFC 0026
//! correction 38). [`Server::open`] returns it as a frame the client decodes, and returns
//! no frame at all for the two failures the protocol fixes no code for — a client too old
//! to parse the frame, and an encoding mismatch — because `rule handshake.rejection` says
//! those close without one.
//!
//! # Where the `Opaque` payloads become real bytes
//!
//! `daemon` emits a typed [`Payload`] beside a `ResultEnvelope` whose `payload` reads
//! `null`, because the operation layer has no codec by design. This module is where the
//! two meet: [`Server::serve`] encodes the typed payload into the envelope's `payload`
//! field before writing the frame, so a client reads one message with the response inside
//! it rather than a message plus an out-of-band value. `payload` is `null` on the wire
//! exactly where the IDL says it must be — on `status = error`.
//!
//! # Which frames the negotiated encoding governs
//!
//! "Exactly one [encoding] is negotiated per connection" (IDL §2), so [`Server::answer`]
//! reads a request frame and writes a result frame in [`Negotiated::encoding`] — that is
//! what having negotiated one means, and it is why the encoding is on the connection
//! rather than on the message.
//!
//! The *handshake* frames are the exception, and `rule handshake.bootstrap_encoding` (IDL
//! 1.5) is why: `ClientHello`, `ServerWelcome`, and `ServerReject` are `canonical_json`
//! unconditionally, independent of `ClientHello.encodings` and of what negotiation
//! selects — a `ClientHello` is the frame that offers the encodings, so it cannot already
//! be in the one the offer has not yet selected, and `ServerWelcome`/`ServerReject` carry
//! the negotiation's own outcome before that outcome has anywhere else to apply.
//! [`encode_hello`] and [`Server::open`] therefore stay in `canonical_json`, and the
//! negotiated encoding governs only from the first `RequestEnvelope` onward, which is
//! where [`Server::answer`]'s dispatch on [`Negotiated::encoding`] above picks up.

use std::collections::VecDeque;

use crate::codec::cbor::Cbor;
use crate::codec::json::Json;
use crate::codec::{self, CodecError, Document, read_in, to_bytes, write_in};
use crate::daemon::family::{Arguments, Payload};
use crate::daemon::{Daemon, OperationRequest};
use crate::protocol::envelope::{RequestEnvelope, ResultEnvelope};
use crate::protocol::handshake::{
    ClientHello, Negotiated, NegotiationError, ServerReject, ServerWelcome,
};
use crate::protocol::shared::EvidenceQuery;
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::task::EvidenceEvent;
use crate::protocol::vocabulary::{Encoding, ResultStatus};

/// The largest frame an endpoint will accept.
///
/// A length prefix is read before its payload, so an endpoint that trusted it would let a
/// four-byte frame ask for a four-gigabyte allocation. The bound is the transport's own —
/// `ServerLimits.max_result_bytes` is what a *daemon* declares about results, and this is
/// what an endpoint will read at all — and it is deliberately far above any message this
/// protocol declares.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// The width of the length prefix.
const LENGTH_BYTES: usize = 4;

/// One direction of a connected pair: a byte queue with frame boundaries.
///
/// Bytes are appended whole and read whole, but the reader does not assume they arrived
/// whole: [`Channel::take_frame`] returns [`None`] for a partial frame and leaves the
/// bytes in place, which is the behaviour a stream transport needs and the one a test can
/// exercise by feeding a frame one byte at a time.
#[derive(Debug, Default)]
pub struct Channel {
    bytes: VecDeque<u8>,
}

impl Channel {
    /// An empty channel.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Frame `payload` and append it.
    ///
    /// # Errors
    ///
    /// [`FrameError::TooLarge`] when the payload exceeds [`MAX_FRAME_BYTES`].
    pub fn put_frame(&mut self, payload: &[u8]) -> Result<(), FrameError> {
        let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge)?;
        if payload.len() > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        self.bytes.extend(length.to_be_bytes());
        self.bytes.extend(payload.iter().copied());
        Ok(())
    }

    /// Append raw bytes, as a stream would deliver them.
    pub fn put_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes.iter().copied());
    }

    /// Take the next whole frame, or [`None`] when one has not arrived yet.
    ///
    /// # Errors
    ///
    /// [`FrameError::TooLarge`] when the prefix declares more than [`MAX_FRAME_BYTES`].
    /// The bytes are *not* consumed in that case: an endpoint that cannot read a frame
    /// closes the connection rather than resynchronizing, because a stream whose framing
    /// is in doubt has no next frame to find.
    pub fn take_frame(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        if self.bytes.len() < LENGTH_BYTES {
            return Ok(None);
        }
        let mut prefix = [0_u8; LENGTH_BYTES];
        for (index, slot) in prefix.iter_mut().enumerate() {
            *slot = self.bytes[index];
        }
        let declared = u32::from_be_bytes(prefix) as usize;
        if declared > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        if self.bytes.len() < LENGTH_BYTES + declared {
            return Ok(None);
        }
        self.bytes.drain(..LENGTH_BYTES);
        Ok(Some(self.bytes.drain(..declared).collect()))
    }

    /// Whether anything is buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// A connected pair of endpoints: two channels, one per direction.
///
/// One value owns both, which is what makes an exchange a sequence of calls rather than a
/// schedule. The daemon core is synchronous, so there is nothing to schedule.
#[derive(Debug, Default)]
pub struct LocalPair {
    /// Bytes the client has written and the server has not read.
    pub to_server: Channel,
    /// Bytes the server has written and the client has not read.
    pub to_client: Channel,
}

impl LocalPair {
    /// A connected pair with both directions empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Why a frame could not be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The frame exceeds [`MAX_FRAME_BYTES`].
    TooLarge,
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "the frame exceeds the transport's declared maximum",
        })
    }
}

impl core::error::Error for FrameError {}

/// Why an exchange failed below the operation layer.
///
/// A [`TransportError`] is never an answer to a caller — a caller's answer is a
/// `ResultEnvelope` — except where it names a decode failure, which [`CodecError::code`]
/// maps to the wire code the caller is owed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The frame could not be read or written.
    Frame(FrameError),
    /// The bytes are not the message the frame claims to carry.
    Codec(CodecError),
}

impl From<FrameError> for TransportError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<CodecError> for TransportError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Codec(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for TransportError {}

// --- the client half ----------------------------------------------------------------

/// Encode a `ClientHello` as the connection's first frame.
///
/// # Errors
///
/// [`CodecError`] when the hello cannot be encoded.
pub fn encode_hello(hello: &ClientHello) -> Result<Vec<u8>, CodecError> {
    to_bytes(hello)
}

/// Encode a request: the typed arguments into the envelope's `arguments`, then the whole
/// envelope.
///
/// The envelope arrives with `arguments` already carrying the operation's request struct,
/// or with an empty `Opaque` this function fills in. Filling it in here rather than
/// asking every caller to do it is what makes `rule encoding.opaque_payloads` a property
/// of the transport rather than a convention.
///
/// # Errors
///
/// [`CodecError`] when the arguments or the envelope cannot be encoded.
pub fn encode_request(
    envelope: &RequestEnvelope,
    arguments: &Arguments,
) -> Result<Vec<u8>, CodecError> {
    encode_request_in::<Json>(envelope, arguments)
}

/// Encode a request in the encoding `D`.
///
/// # Errors
///
/// [`CodecError`] when the arguments or the envelope cannot be encoded.
pub fn encode_request_in<D: Document>(
    envelope: &RequestEnvelope,
    arguments: &Arguments,
) -> Result<Vec<u8>, CodecError> {
    let mut envelope = envelope.clone();
    envelope.arguments = encode_arguments_in::<D>(arguments)?;
    write_in::<D, _>(&envelope)
}

/// Encode a typed request body into the `Opaque` the envelope carries.
///
/// # Errors
///
/// [`CodecError`] when the body cannot be encoded.
pub fn encode_arguments(
    arguments: &Arguments,
) -> Result<crate::protocol::scalar::Opaque, CodecError> {
    encode_arguments_in::<Json>(arguments)
}

/// Encode a typed request body into the `Opaque` the envelope carries, in the encoding
/// `D`.
///
/// # Errors
///
/// [`CodecError`] when the body cannot be encoded.
pub fn encode_arguments_in<D: Document>(
    arguments: &Arguments,
) -> Result<crate::protocol::scalar::Opaque, CodecError> {
    match arguments {
        Arguments::WorkspaceCreate(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::WorkspaceFork(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::WorkspaceDiff(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::WorkspaceSeal(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentGet(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentDiff(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentProposeRevision(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentAccept(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentReject(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::IntentLock(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::EvidenceGet(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::EvidenceQuery(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::EvidenceVerify(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::EvidenceSubscribe(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::EvidenceLink(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::ObserveIngest(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::ObserveClassify(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::ObserveResult(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::VerificationStart(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::VerificationResult(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::VerificationAwait(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::TaskStatus(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::TaskCancel(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::TaskResume(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::TaskSubscribe(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::TaskUpdateBudget(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::ContextCompile(body) => codec::to_opaque_in::<D, _>(body),
        Arguments::ContextExpand(body) => codec::to_opaque_in::<D, _>(body),
    }
}

/// Decode a result frame into the envelope and the typed response beside it.
///
/// The `payload` is decoded through the operation the *caller* invoked, because a result
/// envelope carries `request_id` and not the operation name: RFC 0026's result envelope
/// "carries no `snapshot`, `intent`, `budget`, or trace-correlation field", and the
/// operation is request-side too. A client knows which operation it sent.
///
/// # Errors
///
/// [`CodecError`] when the frame is not a result envelope, or its payload is not that
/// operation's response struct.
pub fn decode_result(
    operation: &str,
    frame: &[u8],
) -> Result<(ResultEnvelope, Payload), CodecError> {
    decode_result_in::<Json>(operation, frame)
}

/// Decode a result frame in the encoding `D`.
///
/// # Errors
///
/// [`CodecError`] when the frame is not a result envelope, or its payload is not that
/// operation's response struct.
pub fn decode_result_in<D: Document>(
    operation: &str,
    frame: &[u8],
) -> Result<(ResultEnvelope, Payload), CodecError> {
    let envelope: ResultEnvelope = read_in::<D, _>(frame)?;
    let payload = match &envelope.payload {
        Nullable::Null => Payload::None,
        Nullable::Value(opaque) => codec::operations::decode_payload_in::<D>(operation, opaque)?,
    };
    Ok((envelope, payload))
}

/// What a server-sent frame turned out to be.
///
/// > An event frame IS the declared event struct […] There is no event envelope: a frame
/// > and a `ResultEnvelope` are told apart by their required members, which is the device
/// > `ServerWelcome` and `ServerReject` already use.
/// >
/// > — `rule subscription.delivery`
///
/// A closed pair rather than an `Option<EvidenceEvent>` beside a result, because "this
/// frame was an event" and "this frame was a result with no payload" are different facts
/// and a client branches on them differently.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerFrame {
    /// The answer to a request, with its typed response body beside it.
    ///
    /// Both members are boxed: a `ResultEnvelope` is an order of magnitude wider than an
    /// `EvidenceEvent`, and an enum sized for the rarer variant would make every event frame
    /// a client holds cost what a result costs.
    Result(Box<ResultEnvelope>, Box<Payload>),
    /// A committed evidence-graph delta on a subscription this connection opened.
    Event(EvidenceEvent),
}

/// Decode a server-sent frame as whichever of the two shapes it validates as.
///
/// The discrimination is the required members and nothing else — no tag, no length, no
/// sniffing of the first byte. `ResultEnvelope` requires `request_id` and `status`, which
/// `EvidenceEvent` does not declare; `EvidenceEvent` requires `at` and `kind`, which
/// `ResultEnvelope` does not. So the two decodes are mutually exclusive on every document
/// either side can write, and `transport_local.rs` holds that from both directions rather
/// than trusting this comment.
///
/// The classification is made on the **envelope**, not on the whole result: a frame whose
/// `ResultEnvelope` decodes is a result frame, and if its `payload` is then not `operation`'s
/// response struct that is a decode error about the payload rather than grounds to re-read the
/// frame as an event. Conflating the two would let one client's misnamed operation turn
/// another operation's answer into "an event that failed to parse", which names the wrong
/// shape in the error and hides the real disagreement.
///
/// The result arm is tried first because it is the common frame; the order is a preference,
/// not a rule, and swapping it would change nothing about which frames classify as what.
///
/// # Errors
///
/// [`CodecError`] when the frame is neither shape, or is a result frame whose payload is
/// not `operation`'s response struct.
pub fn decode_server_frame(operation: &str, frame: &[u8]) -> Result<ServerFrame, CodecError> {
    decode_server_frame_in::<Json>(operation, frame)
}

/// Decode a server-sent frame in the encoding `D`.
///
/// # Errors
///
/// [`CodecError`] when the frame is neither shape, or is a result frame whose payload is
/// not `operation`'s response struct.
pub fn decode_server_frame_in<D: Document>(
    operation: &str,
    frame: &[u8],
) -> Result<ServerFrame, CodecError> {
    let Ok(envelope) = read_in::<D, ResultEnvelope>(frame) else {
        // Not a result envelope, so this frame is not a result. If it is not an event
        // either, the *event* decode's error is the one returned: it is the error about the
        // shape this frame was last checked against, and reporting the envelope's error for
        // a frame that was never a result would name the wrong shape.
        return Ok(ServerFrame::Event(read_in::<D, _>(frame)?));
    };
    let payload = match &envelope.payload {
        Nullable::Null => Payload::None,
        Nullable::Value(opaque) => codec::operations::decode_payload_in::<D>(operation, opaque)?,
    };
    Ok(ServerFrame::Result(Box::new(envelope), Box::new(payload)))
}

// --- the server half ----------------------------------------------------------------

/// One evidence subscription open on this connection.
///
/// Two fields, and neither is semantic state. `scope` is the `EvidenceQuery` the client
/// declared and the daemon already answered a frontier for; `delivered` is how far into
/// [`DaemonState::evidence_events`] this connection has been written. The log is the
/// daemon's and is append-only, so the pair is a *cursor* into an authority that outlives
/// the connection — which is why remembering it does not make the connection stateful in
/// the sense INV-002 forbids.
///
/// There is no subscription identity, here or on the wire, because
/// `rule subscription.delivery` gives an event frame no field to carry one: an event
/// carries only what it says about the graph.
///
/// [`DaemonState::evidence_events`]: crate::daemon::state::DaemonState::evidence_events
#[derive(Debug, Clone)]
struct Subscription {
    scope: EvidenceQuery,
    delivered: usize,
}

/// A daemon behind a byte boundary.
///
/// It owns the [`Daemon`] and the connection's [`Negotiated`] settings, and answers
/// frames. The only other thing it holds is the connection's open evidence subscriptions,
/// and [`Subscription`] says why that is still RFC 0026's "no session state": a cursor
/// into an append-only log the daemon owns is not state a client can lose anything by
/// dropping.
#[derive(Debug)]
pub struct Server {
    daemon: Daemon,
    negotiated: Negotiated,
    subscriptions: Vec<Subscription>,
}

impl Server {
    /// Accept a connection whose handshake already succeeded.
    #[must_use]
    pub const fn new(daemon: Daemon, negotiated: Negotiated) -> Self {
        Self {
            daemon,
            negotiated,
            subscriptions: Vec::new(),
        }
    }

    /// How many evidence subscriptions are open on this connection.
    ///
    /// A count rather than the subscriptions themselves: a subscription has no identity a
    /// caller could act on (`rule subscription.delivery`), so what is observable about it
    /// is that it exists.
    #[must_use]
    pub fn subscriptions(&self) -> usize {
        self.subscriptions.len()
    }

    /// The negotiated settings of this connection.
    #[must_use]
    pub const fn negotiated(&self) -> Negotiated {
        self.negotiated
    }

    /// The daemon behind the boundary, for a caller that needs to read its state.
    #[must_use]
    pub const fn daemon(&self) -> &Daemon {
        &self.daemon
    }

    /// Close the connection and give the daemon back.
    ///
    /// The daemon outlives the connection and the subscriptions do not — which is the whole
    /// of "a dropped subscription changes nothing" (`rule subscription.hints_only`) as a
    /// signature: this consumes the [`Server`], so every cursor it held is gone, and what
    /// comes back is the graph, unchanged and still authoritative. A client that reconnects
    /// and re-subscribes gets a frontier covering everything the dropped stream delivered.
    #[must_use]
    pub fn close(self) -> Daemon {
        self.daemon
    }

    /// The daemon behind the boundary, mutably, for out-of-band administration —
    /// capability registration, content staging, model registration. Those are
    /// deliberately not operations (IDL §7), so they are deliberately not frames.
    pub const fn daemon_mut(&mut self) -> &mut Daemon {
        &mut self.daemon
    }

    /// Answer the handshake, as a frame either way.
    ///
    /// Returns the welcome frame's bytes on success and the reject frame's on a refusal
    /// the protocol has a code for. [`None`] is the third outcome and is not an error
    /// either: `rule handshake.rejection` requires the connection to close *without* a
    /// frame when the client's offer does not reach 3.1, and for the two failures the
    /// protocol fixes no code for. A caller that treated [`None`] as a failure to answer
    /// would be inventing the frame the rule withholds.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when the frame cannot be encoded.
    pub fn open(
        welcome: &ServerWelcome,
        reject: Option<&ServerReject>,
        outcome: Result<Negotiated, NegotiationError>,
    ) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(match outcome {
            Ok(_) => Some(to_bytes(welcome)?),
            Err(_) => match reject {
                Some(frame) => Some(to_bytes(frame)?),
                None => None,
            },
        })
    }

    /// Read every whole request frame the client has written, dispatch each, and write a
    /// result frame for each — followed by the event frames the connection's open
    /// subscriptions have come to owe.
    ///
    /// Returns how many requests were served. A partial frame is left in the channel.
    ///
    /// The event frames go out *after* the result of the request that committed the delta,
    /// which is `rule subscription.delivery`'s "written after the operation's initial
    /// response" read at the grain this transport runs at: a delta exists only once the
    /// write that committed it has been answered, so there is no earlier moment to write
    /// one at.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when a frame cannot be read or a result cannot be encoded. A
    /// request that fails to *decode* is not an error here: it is answered with a typed
    /// result envelope carrying the code [`CodecError::code`] names, because a caller
    /// that sent a message this daemon cannot read is still owed an answer.
    pub fn serve(&mut self, pair: &mut LocalPair) -> Result<usize, TransportError> {
        let mut served = 0;
        while let Some(frame) = pair.to_server.take_frame()? {
            let response = self.answer(&frame)?;
            pair.to_client.put_frame(&response)?;
            for event in self.deliver()? {
                pair.to_client.put_frame(&event)?;
            }
            served += 1;
        }
        Ok(served)
    }

    /// The event frames this connection owes, encoded, and the cursors advanced past them.
    ///
    /// Empty — and cheap — on a connection with no subscription, which is every connection
    /// that never called `evidence.subscribe`. Calling this twice in a row therefore yields
    /// frames once: the cursors move, and a delta already written is not a delta a second
    /// frame may name (`rule subscription.delivery`, clause 2).
    ///
    /// # Errors
    ///
    /// [`TransportError`] when an event cannot be encoded.
    pub fn deliver(&mut self) -> Result<Vec<Vec<u8>>, TransportError> {
        match self.negotiated.encoding() {
            Encoding::CanonicalJson => self.deliver_in::<Json>(),
            Encoding::CanonicalCbor => self.deliver_in::<Cbor>(),
        }
    }

    /// The delivery itself: one pass over the undelivered tail of the daemon's delta log.
    ///
    /// The **once** in "a delta selected by more than one subscription open on one
    /// connection is delivered once" is what shapes this loop. An event frame carries no
    /// subscription identity, so two frames naming one committed artifact would be
    /// indistinguishable from two committed artifacts — the graph would appear to have
    /// grown by two. So the iteration is over the *log*, not over the subscriptions, and a
    /// delta yields a frame when **any** subscription that was open before it was committed
    /// selects it.
    ///
    /// `subscription.delivered <= index` is the other clause of the same rule: a
    /// subscription delivers only what was committed after it opened, because everything
    /// earlier is the `frontier` its own response carried. Without it a delta would be both
    /// reported as frontier and delivered as a delta, and a client counting artifacts would
    /// double one.
    ///
    /// The cursor then moves to the end of the log **whether or not each delta was in
    /// scope**, and that is deliberate rather than convenient. Holding the unselected ones
    /// back for reconsideration would make a subscription a filter re-run over history —
    /// unbounded work per pass, and a per-delta delivered set instead of a cursor — and the
    /// rule declines it in as many words: "a delta the scope did not select then is not
    /// reconsidered". What a client loses is recoverable by re-reading, which is the whole
    /// content of `rule subscription.hints_only`.
    fn deliver_in<D: Document>(&mut self) -> Result<Vec<Vec<u8>>, TransportError> {
        if self.subscriptions.is_empty() {
            return Ok(Vec::new());
        }
        let state = self.daemon.state();
        let log = state.evidence_events();
        let committed = log.len();
        let mut frames = Vec::new();
        for (index, event) in log.iter().enumerate() {
            let owed = self.subscriptions.iter().any(|subscription| {
                subscription.delivered <= index
                    && crate::daemon::evidence::in_scope(&subscription.scope, event, state)
            });
            if owed {
                frames.push(write_in::<D, _>(event)?);
            }
        }
        for subscription in &mut self.subscriptions {
            subscription.delivered = committed;
        }
        Ok(frames)
    }

    /// Answer one request frame.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when the result cannot be encoded.
    pub fn answer(&mut self, frame: &[u8]) -> Result<Vec<u8>, TransportError> {
        // The connection negotiated an encoding, and this is where having negotiated one
        // means something: the same frame is read and written in it, with no per-message
        // choice and no sniffing of the bytes.
        match self.negotiated.encoding() {
            Encoding::CanonicalJson => self.answer_in::<Json>(frame),
            Encoding::CanonicalCbor => self.answer_in::<Cbor>(frame),
        }
    }

    fn answer_in<D: Document>(&mut self, frame: &[u8]) -> Result<Vec<u8>, TransportError> {
        // The envelope itself not decoding is the one failure with no answer: there is no
        // `request_id` to echo and no operation to check a code against, and inventing a
        // correlation identity for a message that never carried one would be inventing the
        // echo the field exists to make truthful. RFC 0026 leaves a frame a reader cannot
        // parse at the transport level, and so does this.
        let envelope: RequestEnvelope = read_in::<D, _>(frame)?;
        let arguments = match codec::operations::decode_arguments_in::<D>(
            envelope.operation.as_str(),
            &envelope.arguments,
        ) {
            Ok(arguments) => arguments,
            Err(error) => {
                let outcome = self.daemon.refuse(&envelope, error.code());
                return Ok(write_in::<D, _>(&outcome)?);
            }
        };
        // The scope is read off the decoded arguments *before* the dispatch consumes them,
        // and it is used only if the dispatch says `ok` below. Reading it from the request
        // rather than from the response is not a shortcut: the response body is the
        // frontier, and a frontier is a set of handles, not the filter that produced it.
        let scope = match &arguments {
            Arguments::EvidenceSubscribe(request) => Some(request.scope.clone()),
            _ => None,
        };
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope,
            arguments,
        });
        // A subscription exists exactly when the daemon answered one. A refusal — a denied
        // capability, or a deployment that does not serve the channel — leaves the
        // connection with nothing to deliver on, which is the difference between a client
        // that is subscribed and one that was told it is not.
        if let (Some(scope), ResultStatus::Ok) = (scope, outcome.envelope.status) {
            // The cursor opens at the log's current length, and that boundary is exact
            // rather than approximate: `evidence.subscribe` is `@readonly` and commits
            // nothing, and a dispatch holds `&mut DaemonState` for its whole extent, so
            // "the log as the frontier saw it" and "the log now" are the same log.
            self.subscriptions.push(Subscription {
                scope,
                delivered: self.daemon.state().evidence_events().len(),
            });
        }
        // Where the typed payload becomes the envelope's `payload`. On `status = error`
        // the payload is `Payload::None` and the field stays `null`, which is what the
        // IDL declares for that status.
        let mut result = outcome.envelope;
        result.payload = match codec::operations::encode_payload_in::<D>(&outcome.payload)? {
            Some(opaque) => Nullable::Value(opaque),
            None => Nullable::Null,
        };
        // And where the typed `Error.data` becomes the error's `data` — the same seam for
        // the same reason: an `Opaque` carries bytes of the *negotiated* encoding, so it
        // is encoded here and not in the encoding-free dispatch (RFC 0026 F19, protocol
        // 3.4). `ErrorData::None` leaves the field absent, which is what
        // `rule encoding.opaque_payloads` requires of every code with no declared shape.
        if let Some(opaque) = codec::operations::encode_error_data_in::<D>(&outcome.data)? {
            if let Optional::Present(error) = &mut result.error {
                error.data = Optional::Present(opaque);
            } else {
                debug_assert!(
                    false,
                    "a typed `Error.data` value can only arrive on a failure outcome"
                );
            }
        }
        Ok(write_in::<D, _>(&result)?)
    }
}

/// Read the next result frame a client has been sent, decoded.
///
/// # Errors
///
/// [`TransportError`] when the frame cannot be read or decoded.
pub fn client_receive(
    pair: &mut LocalPair,
    operation: &str,
) -> Result<Option<(ResultEnvelope, Payload)>, TransportError> {
    client_receive_in::<Json>(pair, operation)
}

/// Read the next result frame a client has been sent, decoded in the encoding `D`.
///
/// # Errors
///
/// [`TransportError`] when the frame cannot be read or decoded.
pub fn client_receive_in<D: Document>(
    pair: &mut LocalPair,
    operation: &str,
) -> Result<Option<(ResultEnvelope, Payload)>, TransportError> {
    let Some(frame) = pair.to_client.take_frame()? else {
        return Ok(None);
    };
    Ok(Some(decode_result_in::<D>(operation, &frame)?))
}

/// Read the next frame a client has been sent, as either a result or an event.
///
/// [`client_receive`] is the narrower reader and stays: a client that sent one request and
/// opened no subscription can never be sent an event frame, and asking it to branch on one
/// would be asking it to handle a case its own connection cannot produce.
///
/// # Errors
///
/// [`TransportError`] when the frame cannot be read or is neither shape.
pub fn client_receive_frame(
    pair: &mut LocalPair,
    operation: &str,
) -> Result<Option<ServerFrame>, TransportError> {
    client_receive_frame_in::<Json>(pair, operation)
}

/// Read the next frame a client has been sent, in the encoding `D`.
///
/// # Errors
///
/// [`TransportError`] when the frame cannot be read or is neither shape.
pub fn client_receive_frame_in<D: Document>(
    pair: &mut LocalPair,
    operation: &str,
) -> Result<Option<ServerFrame>, TransportError> {
    let Some(frame) = pair.to_client.take_frame()? else {
        return Ok(None);
    };
    Ok(Some(decode_server_frame_in::<D>(operation, &frame)?))
}

/// Write a request onto the wire, as a client.
///
/// # Errors
///
/// [`TransportError`] when the request cannot be encoded or framed.
pub fn client_send(
    pair: &mut LocalPair,
    envelope: &RequestEnvelope,
    arguments: &Arguments,
) -> Result<(), TransportError> {
    client_send_in::<Json>(pair, envelope, arguments)
}

/// Write a request onto the wire, as a client, in the encoding `D`.
///
/// # Errors
///
/// [`TransportError`] when the request cannot be encoded or framed.
pub fn client_send_in<D: Document>(
    pair: &mut LocalPair,
    envelope: &RequestEnvelope,
    arguments: &Arguments,
) -> Result<(), TransportError> {
    let frame = encode_request_in::<D>(envelope, arguments)?;
    pair.to_server.put_frame(&frame)?;
    Ok(())
}
