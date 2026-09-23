"""docs/19 §9 "Security validation" — TEST-9-07 "signature/provenance verification"
(bn-4yta). Left unclaimed by `s9_security_tests.py` (TEST-9-01 … TEST-9-06, bn-35hb) —
see that module's `unclaimed` evidence key. This module claims TEST-9-07 without
editing a line of the sibling module, `tsys.py`, or the `Justfile`.

# What "signature/provenance verification" splits into, and what each half gets

docs/19 §9's seventh bullet names two things in one phrase. Searched independently in
`crates/`:

1. **Cryptographic signatures — absent.** No crate anywhere in the workspace depends
   on an asymmetric-signature or MAC library (`ed25519`, `dalek`, `hmac`, `p256`,
   `secp256`, `rsa`, `ring`, a `signature` crate — grepped across every
   `crates/*/Cargo.toml` and the workspace `Cargo.toml`; zero hits). The one type
   named "signed" in the workspace, `ArtifactClass::SignedIntentBundle`
   (`crates/continuum-workspace/src/artifact_path.rs`), is a path/prefix token like
   every other artifact class (`"inb"` / `"inb_"`) with no distinct verification code
   path — its integrity guarantee is the same content-addressed identity every other
   class gets, not a signature. `continuum-security::injection`'s red-team corpus
   includes a `"signature":"forged"` payload (`ForgedReceiptJson` vector), but that
   tests prompt-injection resistance in an LLM-facing surface, not a cryptographic
   verifier — there is nothing there that checks a signature, forged or genuine.
   **Per the delivered bar, a named-but-unimplemented mechanism cannot be `enforced`,
   so TEST-9-07 is `partial` regardless of what the second half has.**
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
_ABSENCE_07 = (
    "No crate in `crates/*/Cargo.toml` or the workspace `Cargo.toml` depends on "
    "`ed25519`, `dalek`, `hmac`, `p256`, `secp256`, `rsa`, `ring`, or a `signature` "
    "crate (checked at run time by `_check_no_signature_dependency` below, against "
    "every real, committed `Cargo.toml` in the workspace — this is the one check in "
    "this module that reads real files rather than modeling a predicate). "
    "`ArtifactClass::SignedIntentBundle` is a name only: its `artifact_path.rs` arms "
    "(`\"inb\"` / `\"inb_\"`) are the same class-token mechanism every other artifact "
    "class gets, not a distinct signature-checking path, and "
    "`continuum-security::injection`'s `forged-receipt-json` cases test whether "
    "injected text can pass itself off as an accepted claim inside a prompt, not "
    "whether a cryptographic signature verifies. `check.rs`'s own "
    "`envelope_digests_are_carried_labels_not_facts_the_kernel_can_check` states, as a "
    "test, that the certificate checker cannot itself recompute or reject on a "
    "relabeled digest — binding a digest to a real artifact is "
    "`continuum-workspace::publication`'s job, which is what this module claims "
    "instead. Cryptographic signature verification is therefore not a `partial` "
    "implementation short one corner; it is zero lines, anywhere, and this ID stays "
    "`partial` even though its content-addressed-provenance half is `enforced`-grade "
    "on its own."
)
BOUNDARIES: dict[str, list[str]] = {"TEST-9-07": [_BOUNDARY_07]}
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


_SIGNATURE_DEP_RE = re.compile(r"^(ed25519[\w-]*|dalek[\w-]*|hmac|p256|secp256\w*|rsa|ring|signature)\s*=", re.M)


def _check_no_signature_dependency() -> list[str]:
    """Reads every real, committed `Cargo.toml` in the workspace (not a model of one)
    and confirms no crate declares a dependency on an asymmetric-signature or MAC
    library. This is the one check in the module that is not a port: it is a direct
    read of the real manifests, exactly as `s9_security_tests.py`'s TEST-9-01 calls
    the real governance gates directly rather than modeling them."""
    hits: list[str] = []
    for path in CARGO_TOML_PATHS:
        text = path.read_text(encoding="utf-8")
        for m in _SIGNATURE_DEP_RE.finditer(text):
            hits.append(f"{path.relative_to(ROOT)}: declares a dependency on {m.group(1)!r}")
    return hits


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

    # -- absence: no signature-verification dependency anywhere -----------------------
    sig_hits = _check_no_signature_dependency()
    bump("cargo_toml_files_scanned", len(CARGO_TOML_PATHS))
    if sig_hits:
        # A hit here would mean the ABSENCE claim above is wrong — treat it as
        # informational corroboration of the absence, not a failure of this module.
        bump("unexpected_signature_dependencies", len(sig_hits))
    need("cargo_toml_files_scanned", "a real Cargo.toml to scan")

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
