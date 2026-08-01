//! Version negotiation and the protocol-major N/N−1 service window.
//!
//! > The daemon MUST serve protocol majors N and N−1 concurrently —
//! > `protocol.majors_served = [3, 2]`, observable per connection as
//! > `ServerWelcome.majors_served`. A client on the previous major is rejected only when
//! > it falls outside this window, with `ProtocolVersionUnsupported`.
//! >
//! > The client offers an inclusive `VersionRange`; the daemon selects the **highest
//! > version common** to that range and its served set, or rejects the connection. The
//! > daemon MUST NOT select a version outside the client's range.
//! >
//! > — RFC 0026, "Version window and negotiation"
//!
//! Four cases decide whether the window is implemented or merely described: N is served,
//! N−1 is served, N−2 is refused, N+1 is refused. Each refusal must be typed and must
//! carry `ProtocolVersionUnsupported`; a daemon that refuses N−1 is as wrong as one that
//! accepts N−2.
//!
//! # The 3.0 → 3.1 minor bump
//!
//! `protocol.version` is `"3.1"` as of IDL 1.2, and the whole claim of a *compatible*
//! bump is that a client pinned to 3.0 is unaffected. That claim is a test, not a
//! sentence: `a_client_pinned_to_the_previous_minor_still_gets_it` is it. The window is
//! stated in *majors* (`[3, 2]`), so the bump moves nothing about it — 2.x remains
//! served, which is what N−1 means, and the version-window tests below are unchanged by
//! the minor precisely because a minor is not a window.

use continuum_value::epoch::ProtocolWindow;
use continuumd::protocol::handshake::{
    ClientHello, Negotiated, NegotiationError, ServerReject, VersionRange, negotiate,
};
use continuumd::protocol::registry::{ENCODINGS, MAJORS_SERVED, PROTOCOL_VERSION};
use continuumd::protocol::scalar::{ActorId, CapabilityHandle, ProtocolVersion};
use continuumd::protocol::spec::Optional;
use continuumd::protocol::vocabulary::{Encoding, ErrorCode};

/// The daemon of this test: it implements one retired major and two served ones.
///
/// `1.0` is there on purpose. A daemon that only ever *implements* what it serves cannot
/// tell "you are outside my window" from "we have nothing in common", and the two are
/// different facts about the client.
///
/// `3.0`, `3.1`, and `3.2` are all here for the same reason one level up: a daemon at the
/// new minor still implements the old ones, and a client pinned to `3.0` must still be
/// served `3.0` rather than silently upgraded.
fn implemented() -> Vec<ProtocolVersion> {
    vec![
        ProtocolVersion::new(1, 0),
        ProtocolVersion::new(2, 0),
        ProtocolVersion::new(2, 1),
        ProtocolVersion::new(3, 0),
        ProtocolVersion::new(3, 1),
        ProtocolVersion::new(3, 2),
    ]
}

fn window() -> ProtocolWindow {
    ProtocolWindow::new(3)
}

fn hello(low: ProtocolVersion, high: ProtocolVersion, encodings: Vec<Encoding>) -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange { low, high },
        encodings,
        client: "continuum-test/0".to_owned(),
        actor: ActorId::new("agent:test").expect("a well-formed actor"),
        capability: CapabilityHandle::new("cap_test").expect("a well-formed capability"),
        features: Optional::Absent,
    }
}

fn open(low: (u32, u32), high: (u32, u32)) -> Result<Negotiated, NegotiationError> {
    negotiate(
        &implemented(),
        window(),
        ENCODINGS,
        &hello(
            ProtocolVersion::new(low.0, low.1),
            ProtocolVersion::new(high.0, high.1),
            vec![Encoding::CanonicalCbor, Encoding::CanonicalJson],
        ),
    )
}

#[test]
fn the_declared_window_is_the_idls() {
    assert_eq!(MAJORS_SERVED, [3, 2]);
    let window = window();
    assert!(window.serves(ProtocolVersion::new(3, 0)), "N");
    assert!(window.serves(ProtocolVersion::new(2, 1)), "N−1");
    assert!(!window.serves(ProtocolVersion::new(1, 0)), "N−2");
    assert!(!window.serves(ProtocolVersion::new(4, 0)), "N+1");
}

#[test]
fn a_client_on_the_current_major_is_served() {
    let negotiated = open((3, 0), (3, 1)).expect("N is served");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(3, 1));
}

#[test]
fn the_declared_version_is_the_idls() {
    // `protocol.version` is the version the *document* defines; the conformance test
    // holds this constant to the IDL. This asserts the two facts that make the bump a
    // bump: the constant reads 3.2, and 3.2 is a version the daemon of this test can
    // actually negotiate.
    assert_eq!(PROTOCOL_VERSION, "3.2");
    let declared: ProtocolVersion = PROTOCOL_VERSION.parse().expect("canonical");
    assert!(implemented().contains(&declared));
    assert!(window().serves(declared));
}

#[test]
fn a_client_pinned_to_the_previous_minor_still_gets_it() {
    // The compatibility claim of a minor bump, stated as a test: 3.0 is still offered,
    // still selected when the client asks for exactly it, and is not silently upgraded to
    // 3.1. `rule versioning.compatible_change` then forbids the daemon from emitting any
    // 3.1 field on that connection.
    let negotiated = open((3, 0), (3, 0)).expect("3.0 is still implemented and served");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(3, 0));
    assert!(negotiated.admits_request(ProtocolVersion::new(3, 0)));
    assert!(
        !negotiated.admits_request(ProtocolVersion::new(3, 1)),
        "a connection negotiated at 3.0 does not admit a 3.1 request"
    );
}

#[test]
fn a_client_on_the_previous_major_is_served() {
    let negotiated = open((2, 0), (2, 1)).expect("N−1 is served");
    assert_eq!(
        negotiated.protocol_version(),
        ProtocolVersion::new(2, 1),
        "the highest common version, not the lowest"
    );
}

#[test]
fn a_client_two_majors_back_is_refused_with_a_typed_error() {
    let error = open((1, 0), (1, 0)).expect_err("N−2 is outside the window");
    assert_eq!(error, NegotiationError::OutsideServiceWindow);
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::ProtocolVersionUnsupported)
    );
}

#[test]
fn a_client_on_the_next_major_is_refused_with_a_typed_error() {
    let error = open((4, 0), (4, 9)).expect_err("N+1 is not implemented");
    assert_eq!(error, NegotiationError::NoCommonVersion);
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::ProtocolVersionUnsupported)
    );
}

#[test]
fn a_range_spanning_the_window_selects_the_highest_served_version() {
    // The client would accept anything from 1.0 to 9.9. The daemon must not hand back
    // 1.0 (outside its window) or 4.0 (which it does not implement), and must prefer the
    // newest minor it implements inside the window.
    let negotiated = open((1, 0), (9, 9)).expect("3.2 is common and served");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(3, 2));
}

#[test]
fn the_daemon_never_selects_outside_the_clients_range() {
    // A client pinned to exactly 2.0 gets 2.0, never the newer 2.1 or 3.0.
    let negotiated = open((2, 0), (2, 0)).expect("2.0 is served");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(2, 0));
}

#[test]
fn an_inverted_range_is_rejected_rather_than_normalized() {
    let error = open((3, 0), (2, 0)).expect_err("high precedes low");
    assert_eq!(error, NegotiationError::MalformedRange);
    // The IDL fixes no wire code for a malformed handshake frame; see
    // `NegotiationError::error_code`.
    assert_eq!(error.error_code(), None);
}

#[test]
fn the_clients_encoding_preference_decides() {
    let cbor_first = negotiate(
        &implemented(),
        window(),
        ENCODINGS,
        &hello(
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 0),
            vec![Encoding::CanonicalCbor, Encoding::CanonicalJson],
        ),
    )
    .expect("served");
    assert_eq!(cbor_first.encoding(), Encoding::CanonicalCbor);

    let json_first = negotiate(
        &implemented(),
        window(),
        ENCODINGS,
        &hello(
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 0),
            vec![Encoding::CanonicalJson, Encoding::CanonicalCbor],
        ),
    )
    .expect("served");
    assert_eq!(json_first.encoding(), Encoding::CanonicalJson);
}

#[test]
fn a_daemon_that_shares_no_encoding_refuses_the_connection() {
    let error = negotiate(
        &implemented(),
        window(),
        &[Encoding::CanonicalCbor],
        &hello(
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 0),
            vec![Encoding::CanonicalJson],
        ),
    )
    .expect_err("no encoding in common");
    assert_eq!(error, NegotiationError::NoCommonEncoding);
    assert_eq!(error.error_code(), None);
}

#[test]
fn a_client_offering_no_encoding_is_refused() {
    let error = negotiate(
        &implemented(),
        window(),
        ENCODINGS,
        &hello(
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 0),
            Vec::new(),
        ),
    )
    .expect_err("an empty preference list shares nothing");
    assert_eq!(error, NegotiationError::NoCommonEncoding);
}

#[test]
fn a_request_naming_another_version_is_not_admitted() {
    // > A `RequestEnvelope` naming a different `protocol_version` than the connection
    // > negotiated MUST be rejected with `ProtocolVersionUnsupported`. Re-negotiation
    // > mid-connection is not a protocol feature.
    let negotiated = open((3, 0), (3, 0)).expect("served");
    assert!(negotiated.admits_request(ProtocolVersion::new(3, 0)));
    assert!(
        !negotiated.admits_request(ProtocolVersion::new(3, 1)),
        "a minor difference is a different version"
    );
    assert!(!negotiated.admits_request(ProtocolVersion::new(2, 1)));
}

// --- the 3.1 rejection frame ---------------------------------------------------------

/// A hello offering `low..=high` in both encodings.
fn hello_at(low: (u32, u32), high: (u32, u32)) -> ClientHello {
    hello(
        ProtocolVersion::new(low.0, low.1),
        ProtocolVersion::new(high.0, high.1),
        vec![Encoding::CanonicalCbor, Encoding::CanonicalJson],
    )
}

/// The refusal `hello` would produce, if the daemon sends one at all.
fn reject(hello: &ClientHello) -> Option<ServerReject> {
    let error = negotiate(&implemented(), window(), ENCODINGS, hello)
        .expect_err("this helper is for refusals");
    ServerReject::for_negotiation(error, hello, MAJORS_SERVED, &error.to_string())
}

#[test]
fn a_refused_client_that_can_read_the_frame_gets_a_typed_refusal() {
    // RFC 0027 F4: before 3.1 nothing carried a connection-level
    // `ProtocolVersionUnsupported`, so a client could not tell a typed refusal from a
    // network failure. This is that flag paid. The client here offers 4.0–4.9, a range
    // that reaches past 3.1 (so it can parse the frame) and shares no version with the
    // daemon (so it is refused).
    let unservable = hello_at((4, 0), (4, 9));
    let frame = reject(&unservable).expect("a client reaching 3.1 gets the frame");
    assert_eq!(frame.code, ErrorCode::ProtocolVersionUnsupported);
    assert!(
        !frame.retryable,
        "every code this version admits is non-retryable"
    );
    assert_eq!(
        frame.majors_served, MAJORS_SERVED,
        "the window is readable from the refusal, which is the client's only channel for it"
    );
}

#[test]
fn a_client_too_old_to_parse_the_frame_is_closed_without_one() {
    // > A daemon MUST send it only when the client's offered `VersionRange` reaches a
    // > version that defines it — `high` at or above `"3.1"` — and MUST close the
    // > connection without a frame otherwise, because a frame the client cannot parse is
    // > not a typed refusal.
    // >
    // > — `rule handshake.rejection`
    assert!(
        reject(&hello_at((4, 0), (4, 9))).is_some(),
        "a client above 3.1 can parse the frame"
    );
    assert!(
        reject(&hello_at((1, 0), (1, 0))).is_none(),
        "a client whose whole range predates 3.1 cannot parse the frame"
    );
    assert!(
        reject(&hello_at((2, 9), (2, 9))).is_none(),
        "and neither can one on the previous major that never offered 3.1"
    );
}

#[test]
fn a_refusal_the_protocol_fixes_no_code_for_sends_no_frame() {
    // `rule handshake.rejection` names the admissible set as exactly
    // `ProtocolVersionUnsupported` and `CapabilityDenied`. A malformed range and an
    // encoding mismatch are outside it, so they stay transport-level closes rather than
    // acquiring an invented code.
    let inverted = hello_at((3, 1), (2, 0));
    assert_eq!(
        negotiate(&implemented(), window(), ENCODINGS, &inverted),
        Err(NegotiationError::MalformedRange)
    );
    assert!(reject(&inverted).is_none());

    let no_encoding = hello(
        ProtocolVersion::new(3, 1),
        ProtocolVersion::new(3, 1),
        vec![Encoding::CanonicalJson],
    );
    let error = negotiate(
        &implemented(),
        window(),
        &[Encoding::CanonicalCbor],
        &no_encoding,
    )
    .expect_err("no encoding in common");
    assert_eq!(error, NegotiationError::NoCommonEncoding);
    assert!(
        ServerReject::for_negotiation(error, &no_encoding, MAJORS_SERVED, &error.to_string())
            .is_none()
    );
}

#[test]
fn the_protocol_version_spelling_round_trips() {
    // `ProtocolVersion` is `continuum_value`'s `ProtocolEpoch`, whose parse/render round
    // trip is the IDL's `@pattern`. Leading zeros and extra separators are not versions.
    let parsed: ProtocolVersion = "3.0".parse().expect("canonical");
    assert_eq!(parsed, ProtocolVersion::new(3, 0));
    assert_eq!(parsed.to_string(), "3.0");
    for bad in ["3", "3.0.0", "03.0", "3.00", "", "3.", ".0", "3.0 ", "v3.0"] {
        assert!(bad.parse::<ProtocolVersion>().is_err(), "{bad:?}");
    }
}
