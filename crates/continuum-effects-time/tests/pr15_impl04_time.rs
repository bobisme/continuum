//! PR-15 / IMPL-04 (bn-1oj6): the `time/unmodelled-v0` profile, which declares every
//! clock semantic RFC 0002 names as unsupported, with no operation that asks for one.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl04-time-hon-01` | [`honesty_the_profile_covers_every_rfc_0002_clock_semantic`] | every RFC 0002 clock bullet has one row, in order, and a new bullet fails the test |
//! | `pr15-impl04-time-hon-02` | [`honesty_every_row_is_unsupported_with_one_no_operation_case`] | every row is unsupported with a reason and one declared case; compiler-level: `Request` and `Reliance` are matched exhaustively, so no case can be requestable |
//! | `pr15-impl04-time-hon-03` | [`profile_declares_host_qualification_none`] | T06: the profile claims no host semantics |
//! | `pr15-impl04-time-hon-04` | [`honesty_the_profile_bytes_are_pinned_to_its_version`] | a profile edit without a version bump fails |

use std::collections::BTreeSet;

use continuum_effects_time::profile::{
    HostQualification, Operations, PROFILE_NAME, PROFILE_VERSION,
};
use continuum_effects_time::{
    FidelityClass, Reliance, Request, Semantic, Support, UNMODELLED_V0, UNSUPPORTED,
};

const RFC_0002: &str =
    include_str!("../../../notes/plan/rfcs/0002-controlled-effects-and-domain-packs.md");

fn section<'t>(text: &'t str, heading: &str) -> &'t str {
    let start = text.find(heading).expect("heading present") + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n#").unwrap_or(rest.len());
    &rest[..end]
}

/// `pr15-impl04-time-hon-01`. Every clock semantic RFC 0002 "Clock" names is one row,
/// in the RFC's order, with a distinct token.
#[test]
fn honesty_the_profile_covers_every_rfc_0002_clock_semantic() {
    use Semantic as S;
    let rfc: &[(&str, Semantic)] = &[
        ("monotonic local time", S::MonotonicLocalTime),
        ("wall time", S::WallTime),
        ("drift and skew", S::DriftAndSkew),
        ("discontinuities", S::Discontinuities),
        ("uncertainty intervals", S::UncertaintyIntervals),
        ("lease assumptions", S::LeaseAssumptions),
        (
            "timer coalescing and delayed wakeup",
            S::TimerCoalescingAndDelayedWakeup,
        ),
    ];
    let clock = section(RFC_0002, "### Clock");
    let bullets: Vec<&str> = clock
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| line.trim_end_matches([';', '.']))
        .collect();
    assert_eq!(
        bullets,
        rfc.iter().map(|(bullet, _)| *bullet).collect::<Vec<_>>(),
        "RFC 0002's clock list changed; state the new semantic in the profile"
    );
    assert_eq!(
        rfc.iter().map(|(_, row)| *row).collect::<Vec<_>>(),
        Semantic::ALL
    );
    let tokens: BTreeSet<&str> = Semantic::ALL.iter().map(|s| s.token()).collect();
    assert_eq!(tokens.len(), Semantic::ALL.len());
    for (bullet, row) in rfc {
        assert_eq!(row.token(), bullet.replace(' ', "-"));
    }
}

/// What a caller could ask for. Matched with no wildcard arm: a requestable variant
/// added to [`Request`] stops this test compiling.
const fn is_requestable(request: Request) -> bool {
    match request {
        Request::NoOperation => false,
    }
}

/// What a verdict says. Matched with no wildcard arm: a new [`Reliance`] variant stops
/// this test compiling.
const fn is_outside_pack(reliance: Reliance) -> bool {
    match reliance {
        Reliance::OutsidePack => true,
    }
}

/// `pr15-impl04-time-hon-02`. Every row is unsupported with a reason and has exactly
/// one declared case, in order. No case is requestable and every case is outside the
/// pack, and both facts are held by the compiler through exhaustive matches. That the
/// crate has no operation that takes input at all is the recorded INV-015 inventory's
/// claim (`crates/continuumd/tests/inv015_agent_least_authority_evidence.rs`), which
/// is lexical.
#[test]
fn honesty_every_row_is_unsupported_with_one_no_operation_case() {
    let declared: Vec<Semantic> = UNSUPPORTED.iter().map(|c| c.semantic).collect();
    assert_eq!(declared, Semantic::ALL);
    for semantic in Semantic::ALL {
        assert_eq!(semantic.support(), Support::Unsupported, "{semantic}");
        assert!(semantic.statement().len() > 30, "{semantic}");
        let case = semantic.unsupported_case().expect("a declared case");
        assert_eq!(case.semantic, semantic);
        assert!(case.host_behaviour.len() > 30, "{semantic}");
        assert!(!is_requestable(case.request), "{semantic}");
        assert!(is_outside_pack(case.reliance), "{semantic}");
    }
    // The host behaviours the lead's brief names are among the declared ones.
    let text = |s: Semantic| s.unsupported_case().unwrap().host_behaviour;
    assert!(text(Semantic::Discontinuities).contains("leap second"));
    assert!(text(Semantic::Discontinuities).contains("goes backwards"));
    assert!(text(Semantic::DriftAndSkew).contains("skew between"));
}

/// `pr15-impl04-time-hon-03`. T06: the declared profile claims no host semantics. The
/// absence of host effects themselves is the compiler's to prove: the crate is
/// `#![no_std]`, and `tests/pr15_no_std_lane.rs` shows that proof is live.
#[test]
fn profile_declares_host_qualification_none() {
    assert_eq!(UNMODELLED_V0.host, HostQualification::None);
    assert_eq!(UNMODELLED_V0.class, FidelityClass::AdversarialEnvelope);
    assert_ne!(UNMODELLED_V0.class, FidelityClass::PlatformQualified);
    assert_eq!(UNMODELLED_V0.name, PROFILE_NAME);
    // The class covers nothing, and the profile says so in its canonical bytes.
    assert_eq!(UNMODELLED_V0.operations, Operations::None);
    let bytes = UNMODELLED_V0.canonical_bytes();
    assert!(bytes.windows(15).any(|w| w == b"operations-none"));
}

const PINNED_FINGERPRINT: u64 = 16_089_570_159_817_647_713;

/// `pr15-impl04-time-hon-04`. The profile's canonical bytes are pinned together with
/// its version (docs/17 §12). Editing a row, a statement or a declared case changes the
/// fingerprint and fails this test. The rule is to bump `PROFILE_VERSION` and re-pin
/// both.
#[test]
fn honesty_the_profile_bytes_are_pinned_to_its_version() {
    let bytes = UNMODELLED_V0.canonical_bytes();
    let fingerprint = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(
        (PROFILE_VERSION.to_string(), fingerprint),
        ("0.1.0".to_owned(), PINNED_FINGERPRINT),
        "time/unmodelled-v0 changed: a content change needs a new profile name (RFC 0002 correction 1)"
    );
    let name = PROFILE_NAME.as_bytes();
    assert!(bytes.windows(name.len()).any(|w| w == name));
}
