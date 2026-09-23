"""docs/19 §9 "Security validation" — TEST-9-07 "signature/provenance verification"
(bn-4yta). Left unclaimed by `s9_security_tests.py` (TEST-9-01 … TEST-9-06, bn-35hb) —
see that module's `unclaimed` evidence key. This module claims TEST-9-07 without
editing a line of the sibling module, `tsys.py`, or the `Justfile`.

# What "signature/provenance verification" splits into, and what each half gets

docs/19 §9's seventh bullet names two things in one phrase. Searched independently in
`crates/`:

1. **Cryptographic signatures — library landed, production use absent.** bn-2ee4c
   (ADR-0054) added `continuum-evidence::signing`: Ed25519 (`ed25519-dalek` `=2.2.0`,
   strict verification) over a canonical envelope that binds the artifact's ADR-0013
   canonical bytes, the kind, and the signer. Its `SignatureVerifier` is total: a
   missing, malformed, kind-relabelled, tampered, wrong-key, stale-registry,
   unknown-standing, revoked, or not-allowed signature is typed unverified provenance
   (INV-008), never a pass, and `verify_for_ci_acceptance` turns each into
   `AcceptanceChainInvalid`. `_check_signature_verdict` ports that decision order;
   `_check_signing_drift` ties the port to the real `fn check` body; the named Rust
   tests exercise the real code (RFC 8032 known-answer vectors among them); and
   `_check_signature_dependency_placement` reads every real `Cargo.toml`.
   **But no production path calls it** (review cr-3e3t1j): no real receipt, intent
   bundle, or domain pack is signed or verified outside the module's tests. The daemon
   wiring and `intent.accept`'s fail-closed check are bn-3glnv; the OS entropy source,
   keystore, and producer-side signing are bn-1hape. A mechanism real systems do not
   use is not "signature verification" of their artifacts, so this ID stays `partial`.
2. **Content-addressed provenance — real, landed, non-stub.** Two production
   mechanisms bind an artifact's claimed identity to its actual bytes and reject a
   forged, tampered, or substituted claim:
   - `continuum-value::identity` (ADR-0013): `ContentIdentity` and `Digest256` make a
     digest a pure function of canonical bytes; a non-canonical encoding, a malformed
     digest token, or an uppercase/mis-length token is a typed rejection, never a
     silent acceptance.
   - `continuum-workspace::publication` (`Published<H>`, `ReferenceStore`): the store
     computes an artifact's identity from its own content — "a lying publisher cannot
     file content under someone else's name" (the module's own doc comment) — and
     `Published::attest` mints a "this artifact is published" witness only from a
     receipt whose handle the offered name actually equals. Two different contents
     colliding on one claimed identity abort rather than conflate; two publications of
     identical content converge to one identity rather than mint a second.
   Both are drift-tied below to their real, committed source, and both are
   corroborated by real, non-`#[ignore]`d Rust `#[test]` functions.

TEST-9-07 is `partial` until bn-3glnv and bn-1hape put the signing library on a
production path; `_check_no_production_signing_caller` below keeps that absence
checked, so the status cannot silently outlive it.

Certificate wire-form checking (INV-004) was also searched:
`crates/continuum-engine-reference/src/certificate.rs` writes the certificate and
`crates/continuum-kernel-core/src/check.rs` checks it from wire bytes alone, exactly
as plan §20 requires. But the kernel's own test states the boundary this module must
not cross past: `envelope_digests_are_carried_labels_not_facts_the_kernel_can_check`
(`check.rs`) relabels `model_digest` on a certificate and asserts the checker still
*verifies* — "changing a digest byte produces a different verified claim, never a
rejection, because nothing in the certificate lets the kernel recompute it." So the
certificate checker is not itself a provenance-forgery detector; binding a digest to a
real artifact is `continuum-workspace::publication`'s job (the module doc names this
outright: "Binding those digests to real artifacts is the receipt's obligation"). This
module therefore claims the publication layer, not the certificate checker, as its
provenance evidence, and does not claim `envelope_digests_are_carried_labels…` as
positive evidence — it is cited only in `ABSENCE` below.

# Method (same discipline as the sibling module)

`check_fixture` is a Python reference model of three real, narrow properties, each
drift-tied to its real source by extracting a constant or checking a literal phrase is
still present at run time (never hand-copied). None of this executes
`Value::encode`, `Blake3Hasher`, or `ReferenceStore::publish` (no cargo) — the models
are structural (equality, length, character class), not a reimplementation of the real
hash functions, and the real, non-`#[ignore]`d Rust tests named below are the evidence
that the real code has the property the model states.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 9
TITLE = "Security validation"

# tools/test-policy/sections/s9_security_tests_07.py -> repo root
ROOT = Path(__file__).resolve().parents[3]
PUBLICATION_RS_PATH = ROOT / "crates/continuum-workspace/src/publication.rs"
IDENTITY_RS_PATH = ROOT / "crates/continuum-value/src/identity.rs"
CHECK_RS_PATH = ROOT / "crates/continuum-kernel-core/src/check.rs"
SIGNING_RS_PATH = ROOT / "crates/continuum-evidence/src/signing.rs"
SIGNING_TESTS_PATH = ROOT / "crates/continuum-evidence/tests/signing_identities.rs"
CARGO_TOML_PATHS = sorted((ROOT / "crates").glob("*/Cargo.toml")) + [ROOT / "Cargo.toml"]

OBLIGATIONS = {
    "TEST-9-07": "signature/provenance verification.",
}

RULES: dict[str, str] = {
    "publication-name-substitution-not-rejected-detected": "TEST-9-07",
    "identity-collision-not-aborted-detected": "TEST-9-07",
    "distinct-artifacts-conflated-detected": "TEST-9-07",
    "identical-content-not-converged-detected": "TEST-9-07",
    "digest-token-tamper-not-rejected-detected": "TEST-9-07",
    "unverified-signature-accepted-detected": "TEST-9-07",
    "verified-signature-rejected-detected": "TEST-9-07",
    "ci-acceptance-failed-open-detected": "TEST-9-07",
}

STATUS: dict[str, str] = {
    "TEST-9-07": "partial",
}

_BOUNDARY_07 = (
    "`_check_publication_attest` ports `PublishedName::names`/`Published::attest` "
    "(`publication.rs`: `fn names(&self, handle: &ArtifactHandle) -> bool { self == "
    "handle }`) as plain equality on (class, identity) — the real predicate needs no "
    "hash to state, so the port is exact, not approximate. `_check_identity_outcome` "
    "ports the store's convergence/collision/distinctness trichotomy structurally "
    "(same content implies same identity; different content sharing a claimed "
    "identity is a collision that must abort; different content with different "
    "identity stands as two artifacts) — this is the property the real "
    "`ContentIdentifier::identify` doc contract states ('a pure function … two calls "
    "with equal arguments … must return equal handles', INV-005/INV-006) and that "
    "`an_identity_collision_aborts_rather_than_conflating_two_artifacts` and "
    "`distinct_artifacts_never_share_identity` corroborate; it is independent of "
    "which hash algorithm the real `ContentIdentifier` fixture uses; this module "
    "never computes one. `_check_digest_token` ports `Digest256::from_token` "
    "(`identity.rs`) as a length-and-character-class check, drift-tied by extracting "
    "`DIGEST_BITS` at run time and confirming the `DIGEST_LEN * 2`/`DIGEST_BITS / 8` "
    "arithmetic phrases are still present, so a change to the digest width is "
    "noticed, not just a change to the accept/reject outcome. What this cannot do: "
    "compute a real `blake3-256` digest, run `Value::encode`'s canonical-encoding "
    "rules, or exercise `ReferenceStore` concurrency itself (no cargo) — only the "
    "structural properties named above, which is what the named real Rust tests "
    "exercise for real."
)
_BOUNDARY_07_SIGNATURE = (
    "`_check_signature_verdict` ports `SignatureVerifier::check` (`signing.rs`) as its "
    "decision order over eight observable facts — present, well-formed, kind matches, "
    "authentic under the claimed key, registry current against the authoritative head, "
    "signer standing known, signer not revoked, signer allowed for the kind — and "
    "the two policies over it: `verify` (any failure is typed unverified provenance with "
    "the first failing reason) and `verify_for_ci_acceptance` (any failure is "
    "`AcceptanceChainInvalid`). The port is exact about order and outcome, and "
    "`_check_signing_drift` confirms at run time that the real `fn check` body still "
    "tests those facts in that order and that the real verifier still calls "
    "`verify_strict`. What it cannot do: compute an Ed25519 signature or a canonical "
    "encoding (no cargo). That is what the named Rust tests do for real, including the "
    "RFC 8032 §7.1 known-answer vectors and a pinned envelope signature."
)
_ABSENCE_07 = (
    "Production use of signatures is absent (review cr-3e3t1j). The library "
    "`continuum-evidence::signing` exists and its 21 named Rust tests below exercise it, "
    "but no crate outside `continuum-evidence` calls `SigningRegistry` or "
    "`SignatureVerifier` (checked at run time by `_check_no_production_signing_caller` "
    "against every real `crates/*/src` file), so no real receipt, intent bundle, or domain "
    "pack is signed or verified. Missing: bn-3glnv — daemon operations and wire form for "
    "signatures, allowed-signers sets and the authoritative registry head, and "
    "`intent.accept` checking a held bundle through `verify_for_ci_acceptance` (protocol "
    "3.6 is frozen); bn-1hape — an OS `KeyEntropy` source in a boundary crate, an on-disk "
    "keystore, and signing inside the receipt, bundle, and pack producers. `check.rs`'s "
    "`envelope_digests_are_carried_labels_not_facts_the_kernel_can_check` still states that "
    "the certificate checker cannot recompute a relabeled digest; that is by design "
    "(INV-004), and the certificate checker does not link the signature crate."
)
BOUNDARIES: dict[str, list[str]] = {"TEST-9-07": [_BOUNDARY_07, _BOUNDARY_07_SIGNATURE]}
ABSENCE: dict[str, str] = {"TEST-9-07": _ABSENCE_07}

# ---------------------------------------------------------------------------
# Real Rust #[test] evidence (textual, no cargo — s6_fuzzing_targets.py's
# _rust_test_present discipline, reused verbatim rather than re-implemented).
# ---------------------------------------------------------------------------

RUST_TESTS: list[tuple[Path, str]] = [
    (PUBLICATION_RS_PATH, "published_attests_only_the_receipts_own_handle"),
    (PUBLICATION_RS_PATH, "published_is_invariant_under_the_handles_serialization_round_trip"),
    (PUBLICATION_RS_PATH, "a_receipt_names_the_identity_the_store_derived"),
    (PUBLICATION_RS_PATH, "distinct_artifacts_never_share_identity"),
    (PUBLICATION_RS_PATH, "an_identity_collision_aborts_rather_than_conflating_two_artifacts"),
    (PUBLICATION_RS_PATH, "concurrent_identical_publication_yields_one_identity_and_every_receipt"),
    (IDENTITY_RS_PATH, "non_canonical_bytes_are_not_an_identity"),
    (IDENTITY_RS_PATH, "malformed_digest_tokens_are_typed_errors"),
    (IDENTITY_RS_PATH, "digest_tokens_round_trip"),
    (IDENTITY_RS_PATH, "blake3_matches_the_published_test_vectors"),
    (SIGNING_RS_PATH, "ed25519_matches_the_rfc_8032_test_vectors"),
    (SIGNING_RS_PATH, "signature_tokens_round_trip_and_reject_malformed_text"),
    (SIGNING_RS_PATH, "weak_and_non_point_public_keys_are_not_signer_identities"),
    (SIGNING_RS_PATH, "an_overlong_token_or_record_is_refused_before_decoding"),
    (SIGNING_RS_PATH, "a_signature_record_fits_the_decode_bound"),
    (SIGNING_TESTS_PATH, "a_signer_with_unknown_standing_is_unverified_not_active"),
    (SIGNING_TESTS_PATH, "a_stale_registry_missing_a_revocation_cannot_verify"),
    (SIGNING_TESTS_PATH, "an_empty_registry_verifies_no_signature"),
    (SIGNING_TESTS_PATH, "sign_verify_round_trips_for_every_signed_kind_from_wire_bytes"),
    (SIGNING_TESTS_PATH, "a_tampered_payload_downgrades_to_signature_mismatch"),
    (SIGNING_TESTS_PATH, "a_signature_under_the_wrong_key_does_not_verify"),
    (SIGNING_TESTS_PATH, "a_revoked_signers_signatures_downgrade_and_it_can_no_longer_sign"),
    (SIGNING_TESTS_PATH, "a_rotated_key_keeps_its_signatures_verifiable_but_cannot_sign_again"),
    (SIGNING_TESTS_PATH, "a_signer_outside_the_allowed_set_or_its_kinds_is_not_trusted"),
    (SIGNING_TESTS_PATH, "a_kind_relabel_is_refused_before_and_after_the_kind_field_is_rewritten"),
    (SIGNING_TESTS_PATH, "an_unverifiable_signature_downgrades_to_typed_unverified_provenance_not_fail_open"),
    (SIGNING_TESTS_PATH, "the_ci_acceptance_check_fails_closed_on_every_unverified_outcome"),
    (SIGNING_TESTS_PATH, "signatures_are_deterministic_and_pinned"),
    (SIGNING_TESTS_PATH, "the_local_key_is_minted_on_first_use_once_and_audited"),
    (SIGNING_TESTS_PATH, "a_lost_key_is_revoked_and_superseded_through_linked_audit_records"),
    (SIGNING_TESTS_PATH, "an_allowed_signers_set_round_trips_through_its_canonical_value"),
]


def _rust_test_present(path: Path, fn_name: str) -> tuple[bool, str]:
    """Textual, no-cargo check: `fn <fn_name>(` exists in `path`, immediately preceded
    (skipping only blank lines and `//`/`///` comments) by an attribute run that
    includes `#[test]` and no `#[ignore]`. Same discipline as the sibling module and
    `s6_fuzzing_targets.py`."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return False, f"could not read {path}: {exc}"
    marker = f"fn {fn_name}("
    at = text.find(marker)
    if at == -1:
        return False, f"{path}: no `fn {fn_name}(` found"
    attrs: list[str] = []
    for line in reversed(text[:at].splitlines()):
        s = line.strip()
        if s.startswith("#["):
            attrs.append(s)
            continue
        if s == "" or s.startswith("///") or s.startswith("//"):
            continue
        break
    if not any(a.startswith("#[test") for a in attrs):
        return False, f"{path}: `fn {fn_name}` is not attributed #[test]"
    if any("ignore" in a for a in attrs):
        return False, f"{path}: `fn {fn_name}` carries #[ignore]"
    return True, ""


def _finding(rule: str, subject: str, message: str) -> dict[str, str]:
    return tsys.Finding(rule, RULES[rule], subject, message).as_json()


# ---------------------------------------------------------------------------
# Drift checks against real source.
# ---------------------------------------------------------------------------

_PUBLISHED_NAME_PHRASE = "fn names(&self, handle: &ArtifactHandle) -> bool {\n        self == handle"
_IDENTITY_COLLISION_PHRASE = "AbortReason::IdentityCollision"
_PURE_FUNCTION_DOC_PHRASE = "publisher cannot file content under someone else's name"


def _check_publication_drift() -> list[str]:
    problems: list[str] = []
    text = PUBLICATION_RS_PATH.read_text(encoding="utf-8")
    if _PUBLISHED_NAME_PHRASE not in text:
        problems.append(
            f"drift: {PUBLICATION_RS_PATH} no longer declares PublishedName::names as plain handle equality"
        )
    if _IDENTITY_COLLISION_PHRASE not in text:
        problems.append(f"drift: {PUBLICATION_RS_PATH} no longer declares {_IDENTITY_COLLISION_PHRASE}")
    if _PURE_FUNCTION_DOC_PHRASE not in text:
        problems.append(f"drift: {PUBLICATION_RS_PATH} no longer states its content-addressing guarantee verbatim")
    return problems


_DIGEST_BITS_RE = re.compile(r"pub const DIGEST_BITS: usize = (\d+);")
_DIGEST_LEN_ARITHMETIC_PHRASE = "pub const DIGEST_LEN: usize = DIGEST_BITS / 8;"
_DIGEST_TOKEN_LEN_ARITHMETIC_PHRASE = "pub const DIGEST_TOKEN_LEN: usize = DIGEST_LEN * 2;"
_DIGEST_TOKEN_CHARCLASS_PHRASE = "'0'..='9' | 'a'..='f'"


def _extract_digest_token_len() -> int:
    text = IDENTITY_RS_PATH.read_text(encoding="utf-8")
    m = _DIGEST_BITS_RE.search(text)
    if not m:
        raise ValueError(f"drift: no DIGEST_BITS constant found in {IDENTITY_RS_PATH}")
    if _DIGEST_LEN_ARITHMETIC_PHRASE not in text or _DIGEST_TOKEN_LEN_ARITHMETIC_PHRASE not in text:
        raise ValueError(f"drift: {IDENTITY_RS_PATH} no longer derives DIGEST_TOKEN_LEN as DIGEST_BITS / 8 * 2")
    if _DIGEST_TOKEN_CHARCLASS_PHRASE not in text:
        raise ValueError(f"drift: {IDENTITY_RS_PATH} no longer restricts a digest token to {_DIGEST_TOKEN_CHARCLASS_PHRASE!r}")
    bits = int(m.group(1))
    return (bits // 8) * 2


_SIGNATURE_DEP_RE = re.compile(r"^(ed25519[\w-]*|dalek[\w-]*|hmac|p256|secp256\w*|rsa|ring|signature)\s*[=.]", re.M)
_SIGNATURE_CRATE = "ed25519-dalek"
_SIGNATURE_PIN_PHRASE = 'ed25519-dalek = { version = "=2.2.0", default-features = false, features = ["zeroize"] }'
_SIGNATURE_CONSUMERS = {"crates/continuum-evidence/Cargo.toml"}
_CHECKER_MANIFESTS = {
    "crates/continuum-certificate/Cargo.toml",
    "crates/continuum-kernel-core/Cargo.toml",
    "crates/continuum-kernel-sat/Cargo.toml",
    "crates/continuum-kernel-smt/Cargo.toml",
    "crates/continuum-kernel-temporal/Cargo.toml",
}


def _signature_dependency_declarations() -> dict[str, list[str]]:
    """Reads every real, committed `Cargo.toml` (not a model of one) and returns, per
    manifest, the signature or MAC crates it declares."""
    found: dict[str, list[str]] = {}
    for path in CARGO_TOML_PATHS:
        text = path.read_text(encoding="utf-8")
        hits = [m.group(1) for m in _SIGNATURE_DEP_RE.finditer(text)]
        if hits:
            found[str(path.relative_to(ROOT))] = hits
    return found


def _check_signature_dependency_placement() -> list[str]:
    """The signature crate is pinned exactly at the workspace root, declared by
    `continuum-evidence` alone, and by no certificate-checker crate (INV-004, ADR-0054
    D3). A direct read of the real manifests, like `s9_security_tests.py`'s TEST-9-01
    calling the real governance gates."""
    problems: list[str] = []
    root_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if _SIGNATURE_PIN_PHRASE not in root_text:
        problems.append(f"drift: the workspace Cargo.toml no longer pins {_SIGNATURE_PIN_PHRASE!r}")
    declared = _signature_dependency_declarations()
    consumers = {path for path in declared if path != "Cargo.toml"}
    for path in sorted(consumers - _SIGNATURE_CONSUMERS):
        problems.append(f"{path} declares {declared[path]!r}; ADR-0054 D3 names continuum-evidence alone")
    for path in sorted(consumers & _CHECKER_MANIFESTS):
        problems.append(f"{path} is a certificate-checker crate and declares a signature crate (INV-004)")
    for path in sorted(_SIGNATURE_CONSUMERS - consumers):
        problems.append(f"{path} no longer declares {_SIGNATURE_CRATE}: the signing half has no implementation")
    return problems


# ---------------------------------------------------------------------------
# TEST-9-07: signature verification (SignatureVerifier::check and its two policies).
# ---------------------------------------------------------------------------

# The facts `fn check` tests, in the order it tests them, each with the unverified
# reason its failure yields. `authentic` is `verify_strict`; `revoked` fails when true.
_SIGNATURE_STEPS: list[tuple[str, str, str]] = [
    ("present", "UnverifiedReason::Unsigned", "unsigned"),
    ("well_formed", "UnverifiedReason::Malformed", "malformed"),
    ("kind_matches", "UnverifiedReason::KindMismatch", "kind-mismatch"),
    ("authentic", "UnverifiedReason::SignatureMismatch", "signature-mismatch"),
    ("registry_current", "UnverifiedReason::StandingStale", "standing-stale"),
    ("standing_known", "UnverifiedReason::StandingUnknown", "standing-unknown"),
    ("not_revoked", "UnverifiedReason::SignerRevoked", "signer-revoked"),
    ("allowed", "UnverifiedReason::SignerNotAllowed", "signer-not-allowed"),
]
_FACTS = [fact for fact, _, _ in _SIGNATURE_STEPS]
_SIGNING_API_RE = re.compile(r"\b(SigningRegistry|SignatureVerifier|LocalKeyring)\b")


def _check_no_production_signing_caller() -> tuple[list[str], list[str]]:
    """The typed absence behind `partial`: no `src/` file of any crate other than
    `continuum-evidence` names the signing API. Returns (callers, problems); a caller
    means the absence is stale and this module needs its successor (bn-3glnv/bn-1hape)."""
    callers: list[str] = []
    for path in sorted((ROOT / "crates").glob("*/src/**/*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        if rel.startswith("crates/continuum-evidence/"):
            continue
        if _SIGNING_API_RE.search(path.read_text(encoding="utf-8")):
            callers.append(rel)
    problems = [
        f"{rel} calls the signing library: production use is no longer absent — re-derive "
        "TEST-9-07's status from that path (bn-3glnv/bn-1hape) instead of this absence"
        for rel in callers
    ]
    return callers, problems
_CHECK_FN_RE = re.compile(r"    fn check\(\n(.*?)\n    }\n", re.S)


def _check_signing_drift() -> list[str]:
    """Ties `_SIGNATURE_STEPS` to the real `fn check` body: each reason's first mention
    (and the `verify_strict` call for authenticity) must appear in the port's order.
    `Malformed` is decided one layer up, in `verify_encoded`, before `check` runs."""
    problems: list[str] = []
    text = SIGNING_RS_PATH.read_text(encoding="utf-8")
    m = _CHECK_FN_RE.search(text)
    if not m:
        return [f"drift: {SIGNING_RS_PATH} no longer declares `fn check(` on SignatureVerifier"]
    body = m.group(1)
    anchors = [
        ("UnverifiedReason::Unsigned", body),
        ("UnverifiedReason::KindMismatch", body),
        (".verify_strict(", body),
        ("UnverifiedReason::StandingStale", body),
        ("UnverifiedReason::StandingUnknown", body),
        ("UnverifiedReason::SignerRevoked", body),
        ("UnverifiedReason::SignerNotAllowed", body),
    ]
    last = -1
    for needle, hay in anchors:
        at = hay.find(needle)
        if at == -1:
            problems.append(f"drift: `fn check` no longer mentions {needle}")
            continue
        if at < last:
            problems.append(f"drift: `fn check` tests {needle} out of the ported order")
        last = at
    if "UnverifiedReason::Malformed(error)" not in text or "fn verify_encoded(" not in text:
        problems.append("drift: `verify_encoded` no longer maps a decode failure to UnverifiedReason::Malformed")
    if "Provenance::Unverified(unverified) => Err(AcceptanceChainInvalid {" not in text:
        problems.append("drift: `verify_for_ci_acceptance` no longer maps every unverified outcome to AcceptanceChainInvalid")
    if "fn check(" in text and "-> Provenance {" not in text:
        problems.append("drift: `verify` no longer returns a total Provenance")
    tests_text = SIGNING_TESTS_PATH.read_text(encoding="utf-8")
    if "const PINNED_SIGNATURE: &str = \"ed25519:" not in tests_text:
        problems.append(f"drift: {SIGNING_TESTS_PATH} no longer pins a known-answer envelope signature")
    return problems


def _signature_reference(facts: dict[str, bool], mode: str) -> str:
    """The real outcome: `verified`, `unverified:<reason>`, or `ci-rejected:<reason>`."""
    for fact, _, reason in _SIGNATURE_STEPS:
        if not facts[fact]:
            return f"{'ci-rejected' if mode == 'ci' else 'unverified'}:{reason}"
    return "verified"


def _check_signature_verdict(payload: dict) -> list[dict[str, str]]:
    facts = {fact: bool(payload[fact]) for fact in _FACTS}
    mode = payload["mode"]
    expected = _signature_reference(facts, mode)
    claimed = payload["claimed_outcome"]
    if claimed == expected:
        return []
    if claimed == "verified":
        rule = "ci-acceptance-failed-open-detected" if mode == "ci" else "unverified-signature-accepted-detected"
    elif expected == "verified":
        rule = "verified-signature-rejected-detected"
    else:
        rule = "ci-acceptance-failed-open-detected" if mode == "ci" else "unverified-signature-accepted-detected"
    return [
        _finding(
            rule,
            ",".join(f"{k}={int(v)}" for k, v in facts.items()) + f";mode={mode}",
            f"real SignatureVerifier outcome is {expected!r} (checks run in the order "
            f"{', '.join(_FACTS)}), fixture claims {claimed!r}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: publication-receipt attestation (Published<H>::attest).
# ---------------------------------------------------------------------------


def _check_publication_attest(payload: dict) -> list[dict[str, str]]:
    receipt = (payload["receipt_class"], payload["receipt_identity"])
    offered = (payload["offered_class"], payload["offered_identity"])
    accepted_ref = receipt == offered
    claimed_accepted = payload["claimed_accepted"]
    if accepted_ref == claimed_accepted:
        return []
    return [
        _finding(
            "publication-name-substitution-not-rejected-detected",
            f"{offered[0]}:{offered[1]} against receipt {receipt[0]}:{receipt[1]}",
            f"offered name {offered!r} against a receipt issued for {receipt!r}: real "
            f"Published::attest acceptance is {accepted_ref} (PublishedName::names is plain "
            f"handle equality), fixture claims accepted={claimed_accepted}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: content-addressed identity — convergence, collision, distinctness.
# ---------------------------------------------------------------------------


def _expected_identity_outcome(same_content: bool, same_identity: bool) -> str:
    if same_content and same_identity:
        return "converged"
    if not same_content and same_identity:
        return "aborted"
    if not same_content and not same_identity:
        return "distinct"
    raise ValueError(
        "contradictory payload: identical content claiming different identities would mean "
        "ContentIdentifier::identify is not a pure function (INV-005/INV-006) — this module "
        "never constructs that combination"
    )


_OUTCOME_RULE = {
    "aborted": "identity-collision-not-aborted-detected",
    "distinct": "distinct-artifacts-conflated-detected",
    "converged": "identical-content-not-converged-detected",
}


def _check_identity_outcome(payload: dict) -> list[dict[str, str]]:
    same_content = payload["content_a"] == payload["content_b"]
    same_identity = payload["identity_a"] == payload["identity_b"]
    expected = _expected_identity_outcome(same_content, same_identity)
    claimed = payload["claimed_outcome"]
    if claimed == expected:
        return []
    rule = _OUTCOME_RULE[expected]
    return [
        _finding(
            rule,
            f"{payload['identity_a']}/{payload['identity_b']}",
            f"content {'equal' if same_content else 'distinct'}, claimed identity "
            f"{'equal' if same_identity else 'distinct'}: real store outcome is {expected!r} "
            f"(a lying publisher cannot file content under someone else's name), fixture "
            f"claims {claimed!r}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: digest token parsing (Digest256::from_token).
# ---------------------------------------------------------------------------

_HEX_DIGITS = set("0123456789abcdef")


def _digest_token_reference(token: str, token_len: int) -> bool:
    if any(c not in _HEX_DIGITS for c in token):
        return False
    return len(token) == token_len


def _check_digest_token(payload: dict) -> list[dict[str, str]]:
    token_len = _extract_digest_token_len()
    token = payload["token"]
    accepted_ref = _digest_token_reference(token, token_len)
    claimed_accepted = payload["claimed_accepted"]
    if accepted_ref == claimed_accepted:
        return []
    return [
        _finding(
            "digest-token-tamper-not-rejected-detected",
            token,
            f"token {token!r} (expected length {token_len}): real Digest256::from_token "
            f"acceptance is {accepted_ref}, fixture claims accepted={claimed_accepted}",
        )
    ]


# ---------------------------------------------------------------------------
# Fixtures dispatch.
# ---------------------------------------------------------------------------


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or "kind" not in system:
        return [
            _finding(
                "publication-name-substitution-not-rejected-detected",
                "<payload>",
                "payload must be an object with a 'kind'",
            )
        ]
    kind = system["kind"]
    payload = {k: v for k, v in system.items() if k != "kind"}
    if kind == "publication-attest":
        return _check_publication_attest(payload)
    if kind == "identity-outcome":
        return _check_identity_outcome(payload)
    if kind == "digest-token":
        return _check_digest_token(payload)
    if kind == "signature-verdict":
        return _check_signature_verdict(payload)
    return [
        _finding(
            "publication-name-substitution-not-rejected-detected",
            "<payload>",
            f"unknown fixture kind {kind!r}",
        )
    ]


# ---------------------------------------------------------------------------
# The real run.
# ---------------------------------------------------------------------------


def real_run() -> dict[str, Any]:
    failures: list[str] = []
    corpus: dict[str, int] = {}
    mutants = {"applied": 0, "detected": 0}

    def bump(key: str, n: int = 1) -> None:
        corpus[key] = corpus.get(key, 0) + n

    def mutate(detected: bool, subject: str) -> None:
        mutants["applied"] += 1
        if detected:
            mutants["detected"] += 1
        else:
            failures.append(f"mutant on {subject} was not detected: the checker reported it clean")

    def need(key: str, what: str) -> None:
        if corpus.get(key, 0) == 0:
            failures.append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    # -- signature crate placement: real manifests ------------------------------------
    bump("cargo_toml_files_scanned", len(CARGO_TOML_PATHS))
    failures.extend(_check_signature_dependency_placement())
    _, caller_problems = _check_no_production_signing_caller()
    failures.extend(caller_problems)
    bump("production_signing_callers_absent", 0 if caller_problems else 1)
    need("production_signing_callers_absent", "a checked absence of production signing callers")
    need("cargo_toml_files_scanned", "a real Cargo.toml to scan")

    # -- signature-verdict: every fact combination, both policies -----------------------
    failures.extend(_check_signing_drift())
    for bits in range(1 << len(_FACTS)):
        facts = {fact: bool(bits >> i & 1) for i, fact in enumerate(_FACTS)}
        for mode in ("provenance", "ci"):
            expected = _signature_reference(facts, mode)
            bump("signature_cases")
            bump(f"signature_{expected.split(':')[0].replace('-', '_')}")
            payload = {**facts, "mode": mode}
            if _check_signature_verdict({**payload, "claimed_outcome": expected}):
                failures.append(f"signature {payload!r}: the reference's own honest claim was flagged")
            wrong = "unverified:signature-mismatch" if expected == "verified" else "verified"
            if mode == "ci" and expected == "verified":
                wrong = "ci-rejected:signature-mismatch"
            mut = _check_signature_verdict({**payload, "claimed_outcome": wrong})
            mutate(bool(mut), f"signature {payload!r} (claimed {wrong!r} instead of {expected!r})")
    for key, what in (
        ("signature_verified", "a signature every check accepts"),
        ("signature_unverified", "a downgraded (unverified) signature"),
        ("signature_ci_rejected", "a CI fail-closed rejection"),
    ):
        need(key, what)

    # -- publication-attest ------------------------------------------------------------
    drift = _check_publication_drift()
    failures.extend(drift)
    attest_cases = [
        ("ev", "aaaa1111", "ev", "aaaa1111", True),
        ("ev", "aaaa1111", "ev", "bbbb2222", False),
        ("ev", "aaaa1111", "task", "aaaa1111", False),
        ("crash", "cp7m3x9", "crash", "cp7m3x9", True),
    ]
    for rc, ri, oc, oi, accepted in attest_cases:
        bump("attest_cases")
        if not accepted:
            bump("attest_substitution_cases")
        payload = {"receipt_class": rc, "receipt_identity": ri, "offered_class": oc, "offered_identity": oi}
        clean = _check_publication_attest({**payload, "claimed_accepted": accepted})
        if clean:
            failures.append(f"attest {payload!r}: the reference's own honest claim was flagged")
        mut = _check_publication_attest({**payload, "claimed_accepted": not accepted})
        mutate(bool(mut), f"attest {payload!r} (claim inverted)")
    need("attest_substitution_cases", "an offered name that is not the receipt's own handle")

    # -- identity-outcome ----------------------------------------------------------------
    outcome_cases = [
        (b"identical", b"identical", "id-a", "id-a", "converged"),
        (b"identical-2", b"identical-2", "id-b", "id-b", "converged"),
        (b"original", b"impostor", "same-id", "same-id", "aborted"),
        (b"first", b"second", "same-id-2", "same-id-2", "aborted"),
        (b"alpha", b"beta", "id-alpha", "id-beta", "distinct"),
        (b"gamma", b"delta", "id-gamma", "id-delta", "distinct"),
    ]
    for ca, cb, ida, idb, outcome in outcome_cases:
        bump(f"outcome_{outcome}")
        payload = {
            "content_a": ca.hex(),
            "content_b": cb.hex(),
            "identity_a": ida,
            "identity_b": idb,
        }
        clean = _check_identity_outcome({**payload, "claimed_outcome": outcome})
        if clean:
            failures.append(f"identity outcome {payload!r}: the reference's own honest claim was flagged")
        for wrong in {"converged", "aborted", "distinct"} - {outcome}:
            mut = _check_identity_outcome({**payload, "claimed_outcome": wrong})
            mutate(bool(mut), f"identity outcome {payload!r} (claimed {wrong!r} instead of {outcome!r})")
    for outcome in ("converged", "aborted", "distinct"):
        need(f"outcome_{outcome}", f"an identity case whose real outcome is {outcome!r}")

    # -- digest-token ----------------------------------------------------------------------
    try:
        token_len = _extract_digest_token_len()
        bump("digest_token_len_extracted")
    except ValueError as exc:
        failures.append(str(exc))
        token_len = 64
    good = "a1" * (token_len // 2)
    token_cases = [
        (good, True),
        ("", False),
        (good[:-1], False),
        (good + "0", False),
        ("A" + good[1:], False),
        (good[:-1] + "g", False),
    ]
    for token, accepted in token_cases:
        bump("digest_token_cases")
        if not accepted:
            bump("digest_token_tamper_cases")
        clean = _check_digest_token({"token": token, "claimed_accepted": accepted})
        if clean:
            failures.append(f"digest token {token!r}: the reference's own honest claim was flagged")
        mut = _check_digest_token({"token": token, "claimed_accepted": not accepted})
        mutate(bool(mut), f"digest token {token!r} (claim inverted)")
    need("digest_token_len_extracted", "a real DIGEST_BITS constant")
    need("digest_token_tamper_cases", "a malformed or tampered digest token")

    # -- named real Rust test evidence -----------------------------------------------------
    rust_tests: list[dict[str, Any]] = []
    for path, fn_name in RUST_TESTS:
        ok, msg = _rust_test_present(path, fn_name)
        rust_tests.append({"path": str(path.relative_to(ROOT)), "test": fn_name, "present": ok})
        bump("rust_tests_named")
        if ok:
            bump("rust_tests_present")
        else:
            failures.append(f"named Rust test evidence missing: {msg}")
    need("rust_tests_present", "named real, existing, non-#[ignore]d Rust test")

    if mutants["applied"] and mutants["detected"] == 0:
        failures.append("TEST-9-07: the mutant was never detected across the corpus (the checker would be vacuous)")

    entry: dict[str, Any] = {
        "status": STATUS["TEST-9-07"],
        "rules": sorted(RULES),
        "failures": failures,
        "corpus": corpus,
        "mutant": mutants,
        "rust_tests": rust_tests,
        "boundaries": BOUNDARIES["TEST-9-07"],
        "absence": ABSENCE["TEST-9-07"],
    }
    return {"requirements": {"TEST-9-07": entry}}
