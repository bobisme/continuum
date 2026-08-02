//! The byte boundary the client speaks across.
//!
//! # Why the client does not own a daemon
//!
//! [`AgentClient`](crate::AgentClient) writes a request frame and reads a result frame. It
//! does not know what is on the other side, and that is the point: a client that held a
//! [`Daemon`] would be measuring itself, and the whole reason PR 10 exists is to measure an
//! *interface*. [`Transport`] is therefore two methods — open, then exchange — and
//! [`LocalLink`] is the in-process implementation of them over `continuumd`'s own
//! [`LocalPair`] and [`Server`].
//!
//! The frame format, the length prefix, the canonical encoding, and the dispatch are all
//! `continuumd::transport`'s. Nothing here reimplements any of them: [`LocalLink`] puts a
//! frame into the client→server channel, asks the server to serve, and takes the frame the
//! server wrote back. What crosses is a `Vec<u8>` neither side built for the other.
//!
//! # What the handshake is, and whose policy it is
//!
//! Version and encoding negotiation is `continuumd::protocol::handshake::negotiate`'s, and
//! *which* versions a deployment implements is the deployment's. A client's whole part in it
//! is to offer a range and read the answer, so [`LocalLink`] is constructed with the welcome
//! frame its deployment already computed (exactly as `continuumd`'s own transport tests
//! compute one) and [`Transport::open`] moves the two real frames across the two real
//! channels. Nothing is simulated: the hello is framed, read back off the wire, and
//! compared byte for byte, and the welcome the client decodes is a `Vec<u8>` it did not
//! build.
//!
//! # What a transport failure is, and what it is not
//!
//! A [`LinkError`] is a failure *below* the protocol: a frame that could not be written, or
//! a server that answered nothing. It is never a refusal — a refusal is a typed
//! [`ResultEnvelope`](continuumd::protocol::envelope::ResultEnvelope) with `status = error`,
//! which arrives as bytes like any other answer and is decoded into a
//! [`Refusal`](crate::Refusal). Keeping the two apart is what stops a client from reporting
//! "the daemon said no" when what happened is "the wire broke".
//!
//! [`Daemon`]: continuumd::daemon::Daemon
//! [`LocalPair`]: continuumd::transport::LocalPair
//! [`Server`]: continuumd::transport::Server

use continuumd::transport::{FrameError, LocalPair, Server, TransportError};

/// A byte boundary: open the connection, then exchange frames over it.
///
/// Two methods, because that is the whole of what a client needs from a connection. A
/// transport that offered more — a subscription, a push channel, a session — would be
/// offering state RFC 0026 gives a connection none of (INV-002).
pub trait Transport {
    /// Write the `ClientHello` frame and return the frame the server answered with.
    ///
    /// # Errors
    ///
    /// [`LinkError`] when the frame cannot be written, or no answer arrives.
    fn open(&mut self, hello: &[u8]) -> Result<Vec<u8>, LinkError>;

    /// Send `frame` and return the frame that came back.
    ///
    /// # Errors
    ///
    /// [`LinkError`] when the frame cannot be written, or no answer arrives.
    fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>, LinkError>;
}

/// Why an exchange failed below the protocol.
///
/// Deliberately small and deliberately not an error *code*: none of these is a wire
/// vocabulary member, because none of them was answered by a daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// The frame could not be written or read.
    Frame(FrameError),
    /// The transport failed below the frame layer.
    Transport(TransportError),
    /// The server served the request but wrote no answer.
    NoAnswer,
    /// The hello frame did not survive the channel it was written to.
    HandshakeCorrupted,
}

impl core::fmt::Display for LinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::NoAnswer => f.write_str("the server wrote no answer frame"),
            Self::HandshakeCorrupted => f.write_str("the hello frame did not survive the channel"),
        }
    }
}

impl core::error::Error for LinkError {}

impl From<FrameError> for LinkError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<TransportError> for LinkError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// The in-process byte boundary: a connected [`LocalPair`], the [`Server`] behind it, and
/// the welcome frame the deployment answers a hello with.
///
/// The first two are borrowed rather than owned, because the caller — a benchmark rig, a
/// test — is what provisions a daemon, and provisioning is administration rather than an
/// operation (IDL §7). A client that owned the server would have to be handed one out of
/// band anyway, and would then be the only route to it.
#[derive(Debug)]
pub struct LocalLink<'a> {
    server: &'a mut Server,
    pair: &'a mut LocalPair,
    welcome: &'a [u8],
}

impl<'a> LocalLink<'a> {
    /// Borrow a server, a connected pair, and a welcome frame as one byte boundary.
    pub const fn new(server: &'a mut Server, pair: &'a mut LocalPair, welcome: &'a [u8]) -> Self {
        Self {
            server,
            pair,
            welcome,
        }
    }
}

impl Transport for LocalLink<'_> {
    fn open(&mut self, hello: &[u8]) -> Result<Vec<u8>, LinkError> {
        self.pair.to_server.put_frame(hello)?;
        let echoed = self
            .pair
            .to_server
            .take_frame()?
            .ok_or(LinkError::NoAnswer)?;
        if echoed != hello {
            return Err(LinkError::HandshakeCorrupted);
        }
        self.pair.to_client.put_frame(self.welcome)?;
        self.pair.to_client.take_frame()?.ok_or(LinkError::NoAnswer)
    }

    fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        self.pair.to_server.put_frame(frame)?;
        self.server.serve(self.pair)?;
        self.pair.to_client.take_frame()?.ok_or(LinkError::NoAnswer)
    }
}
