//! Closed vocabularies fail closed; `@open` ones surface the token and infer nothing.
//!
//! > Enums are closed unless annotated `@open`. A daemon MUST NOT emit a member the
//! > negotiated version does not define. A client receiving an unknown member of a closed
//! > enum MUST treat the message as malformed. A client receiving an unknown member of an
//! > `@open` enum MUST NOT infer semantics from it, MUST NOT crash, and MUST surface it
//! > verbatim to its caller as unknown.
//! >
//! > — `rule versioning.enums`
//!
//! Two obligations, two decode paths, and this file is the evidence for both. Every one
//! of the 33 enums is swept: a vocabulary that is merely *declared* closed but decodes
//! permissively is the drift RFC 0037 calls out — "Forward compatibility is achieved by
//! rejecting, never by ignoring".
//!
//! # Adversarial inputs are linear
//!
//! Every hostile token below is bounded and built by a single `repeat`, never by textual
//! doubling; the guard at the end of `adversarial_tokens` states the bound so a later
//! edit cannot quietly turn this file into a memory test.

use continuumd::protocol::spec::{Open, ProtocolEnum, UnknownMember};
use continuumd::protocol::vocabulary::*;

/// The bound every fixture in this file stays under.
const MAX_FIXTURE: usize = 8192;

/// Tokens no vocabulary defines, each a way a real message goes wrong.
fn adversarial_tokens() -> Vec<String> {
    let tokens = vec![
        String::new(),
        " ".to_owned(),
        "\t".to_owned(),
        "\n".to_owned(),
        "\0".to_owned(),
        "unknown".to_owned(),
        "UNKNOWN".to_owned(),
        "null".to_owned(),
        "undefined".to_owned(),
        "-".to_owned(),
        "_".to_owned(),
        "0".to_owned(),
        "établi".to_owned(),
        "\u{202e}reverse".to_owned(),
        "a".repeat(1024),
        "_".repeat(2048),
        "\0".repeat(512),
        format!("{}{}", "x".repeat(4000), "y"),
    ];
    for token in &tokens {
        assert!(
            token.len() < MAX_FIXTURE,
            "fixtures stay linear and bounded: {} bytes",
            token.len()
        );
    }
    tokens
}

/// Sweep one vocabulary: every declared token decodes to its member, and nothing else
/// decodes at all.
fn sweep<E>()
where
    E: ProtocolEnum + core::fmt::Debug,
{
    assert!(!E::MEMBERS.is_empty(), "{} has members", E::ENUM_NAME);
    assert_eq!(
        E::MEMBERS.len(),
        E::ALL.len(),
        "{}: MEMBERS and ALL are index-aligned",
        E::ENUM_NAME
    );

    for (index, member) in E::MEMBERS.iter().enumerate() {
        let decoded = E::from_wire(member.wire)
            .unwrap_or_else(|_| panic!("{}: {:?} is declared", E::ENUM_NAME, member.wire));
        assert_eq!(
            decoded,
            E::ALL[index],
            "{}: {:?}",
            E::ENUM_NAME,
            member.wire
        );
        assert_eq!(decoded.as_wire(), member.wire, "round trip");

        // Near misses. A vocabulary that accepts these is doing case folding or trimming
        // that no rule authorizes, and two spellings of one token is what ADR-0013
        // forbids.
        for variant in [
            member.wire.to_ascii_uppercase(),
            member.wire.to_ascii_lowercase(),
            format!(" {}", member.wire),
            format!("{} ", member.wire),
            member.wire.replace('-', "_"),
            member.wire.replace('_', "-"),
        ] {
            if variant == member.wire || E::MEMBERS.iter().any(|other| other.wire == variant) {
                continue;
            }
            assert!(
                E::from_wire(&variant).is_err(),
                "{}: {variant:?} is not {:?} and must be rejected",
                E::ENUM_NAME,
                member.wire
            );
        }
    }

    for token in adversarial_tokens() {
        if E::MEMBERS.iter().any(|member| member.wire == token) {
            continue;
        }
        let error = E::from_wire(&token)
            .err()
            .unwrap_or_else(|| panic!("{}: {token:?} must not decode", E::ENUM_NAME));
        assert_eq!(error.enum_name(), E::ENUM_NAME);
        assert_eq!(error.token(), token);
    }
}

macro_rules! sweep_all {
    ($($ty:ty),* $(,)?) => { $( sweep::<$ty>(); )* };
}

#[test]
fn every_vocabulary_decodes_exactly_its_declared_tokens() {
    sweep_all!(
        AuthorityLevel,
        Encoding,
        ResultStatus,
        TaskStatus,
        SemanticVerdict,
        EvaluationVerdict,
        InconclusiveReason,
        EvidenceStatus,
        AssuranceClass,
        EvidenceKind,
        EvidenceNodeKind,
        EvidenceEdgeKind,
        Fragment,
        PolicyDecision,
        OmissionReason,
        RedactionReason,
        Compatibility,
        PriorityClass,
        Audience,
        Portfolio,
        GateName,
        GateStatus,
        ExpansionRelation,
        TargetKind,
        DiagnosticSeverity,
        ErrorCode,
        StructuralOutcome,
        TaskEventKind,
        EvidenceEventKind,
        DiffLayer,
        ExplorationStrategy,
        ExplanationLevel,
        GateProfile,
    );
}

#[test]
fn the_idl_identifier_is_not_a_wire_token() {
    // Four members spell their wire token differently from their identifier. A decoder
    // that accepted the identifier would accept a message no conforming daemon emits.
    assert_eq!(AuthorityLevel::ReviseIntent.as_wire(), "revise-intent");
    assert!(AuthorityLevel::from_wire("revise_intent").is_err());

    assert_eq!(EvidenceKind::DporComplete.as_wire(), "dpor-complete");
    assert!(EvidenceKind::from_wire("dpor_complete").is_err());

    assert_eq!(
        OmissionReason::HeuristicCutoff.as_wire(),
        "heuristic-cutoff"
    );
    assert!(OmissionReason::from_wire("heuristic_cutoff").is_err());

    assert_eq!(GateProfile::PhaseB.as_wire(), "phase-b");
    assert!(GateProfile::from_wire("phase_b").is_err());
}

#[test]
fn exactly_five_vocabularies_are_open_at_protocol_3_1() {
    // > Four enums are `@open` at 3.0 — `ErrorCode`, `TargetKind`, `ExplorationStrategy`,
    // > `ExplanationLevel` — and five at 3.1, which adds `DataGrant`. Every other enum is
    // > closed.
    // >
    // > — RFC 0026, "Versioning and revision"
    //
    // `DataGrant` is `@open` for the same reason `ErrorCode` is, read in the other
    // direction: an unrecognized *grant* is one the holder MUST NOT assume it holds, so
    // the fail-closed reading of an unknown member is already the required one, and
    // closing the enum would make the next grant a breaking change.
    let open: Vec<&str> = continuumd::protocol::registry::ENUMS
        .iter()
        .filter(|spec| spec.open)
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        open,
        [
            "TargetKind",
            "ErrorCode",
            "DataGrant",
            "ExplorationStrategy",
            "ExplanationLevel"
        ]
    );
}

#[test]
fn an_open_vocabulary_surfaces_an_unknown_token_verbatim() {
    // `rule versioning.error_codes`: adding a code is a *minor* change, "because clients
    // are required to treat an unrecognized code as a non-retryable typed failure and
    // MUST NOT infer semantics from it". `Open` is that requirement as a type: there is
    // no path from `Unknown` to a known member.
    let known = Open::<ErrorCode>::decode("StaleSnapshot");
    assert_eq!(known.known(), Some(&ErrorCode::StaleSnapshot));
    assert_eq!(known.unknown(), None);

    for token in adversarial_tokens() {
        let decoded = Open::<ErrorCode>::decode(&token);
        assert_eq!(decoded.unknown(), Some(token.as_str()), "verbatim");
        assert_eq!(decoded.known(), None, "no member is inferred");
    }

    // The same for the other three open vocabularies.
    assert_eq!(
        Open::<TargetKind>::decode("hypergraph").unknown(),
        Some("hypergraph")
    );
    assert_eq!(
        Open::<ExplorationStrategy>::decode("quantum").unknown(),
        Some("quantum")
    );
    assert_eq!(
        Open::<ExplanationLevel>::decode("vibes").unknown(),
        Some("vibes")
    );
}

#[test]
fn a_rejection_never_quotes_the_token_it_rejected() {
    // INV-016 and `rule envelope.no_prose`: a typed error's explanation is stable and
    // non-interpolated. The token came off the wire, so `Display` must not carry it even
    // though the value does.
    let hostile = "\u{202e}drop table evidence; -- and a long tail of attacker prose";
    let error = ErrorCode::from_wire(hostile).expect_err("not a code");
    let rendered = error.to_string();
    assert!(!rendered.contains(hostile), "{rendered}");
    assert!(rendered.contains("ErrorCode"), "{rendered}");
    assert_eq!(
        error.token(),
        hostile,
        "the value keeps it for typed handling"
    );

    let other = UnknownMember::new("ErrorCode", hostile);
    assert_eq!(error, other);
}

#[test]
fn the_result_status_partition_is_closed() {
    // The four statuses are the whole of `ResultStatus`; a fifth would be a breaking
    // change under `rule versioning.breaking_change` (adding a member to a closed enum).
    let tokens: Vec<&str> = ResultStatus::ALL.iter().map(|s| s.as_wire()).collect();
    assert_eq!(tokens, ["ok", "error", "task_started", "task_suspended"]);
    const { assert!(!ResultStatus::OPEN) };
    assert!(ResultStatus::from_wire("pending").is_err());
}
