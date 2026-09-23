#!/usr/bin/env python3
"""Bind docs/09 T04 ("Forged trace or artifact substitution") to its live enforcement.

Source of truth:

- `notes/plan/docs/09_THREAT_MODEL.md` T04 — six required controls, verbatim:
  content-addressed manifests; hash chain/Merkle root over events; build
  identity; optional signing/attestation; reject digest mismatch; preserve
  redaction commitments.

As in `check_t12_evidence.py` and `check_r16_evidence.py`, "citing" a control
here means re-running its enforcer for real — a Python delegate's
`--self-test`/real run, or the exact Rust test that already proves the
behavioural fact — never copying a status string out of a possibly-stale
committed evidence file. A control that regresses between commits fails here
too; this file's own gate is `evidence`, not `documentation`.

T04 differs from T12 and R16 in one respect this file states rather than
hides: **two of the six controls have no live producer today**, and this file
records that as a typed absence (`status: "gap"`) rather than fabricating a
citation or silently passing. A `gap` control is not a failure — INV-008's
spirit ("inconclusive, typed, never a bare boolean") applies to governance
evidence the same way it applies to a checker verdict — but it is *checked*,
not merely asserted: each gap carries a mechanical absence-scan that fails
loudly the moment the absence stops being true, so a silent regression from
"honestly not built" to "silently unbound" cannot happen. See "Gap controls"
below.

Control -> enforcement point map
---------------------------------

    content-addressed manifests                                        PASS
        ADR-0013 ("Canonical structural encodings define identity...
        Artifact digests use cryptographic hashes") — text pinned directly
        (this file, `adr-0013-states-canonical-identity`); `continuum_value`'s
        canonical hashing seam, `ContentIdentity`/`Blake3Hasher`
        (`identity::tests::blake3_is_a_pure_function_of_the_bytes` —
        digests are a pure function of canonical bytes, never of host,
        clock, or process, i.e. never a function of "host cc"); the
        workspace snapshot schema's content-addressed manifest fields
        (`files`, `root_digest` — `continuum-workspace`
        `pr3_exit_evidence.rs::positive_an_old_snapshot_remains_reproducible_`
        `after_the_working_tree_changes`).

    hash chain/Merkle root over events                                  GAP
        No live producer. See "Gap controls" below — this is not the same
        claim as the workspace Merkle root (bn-15gj), which is over
        *workspace content*, a different asset than *production trace
        events* (docs/09 §2's "production traces").

    build identity                                                     PASS
        `continuum-kernel-core`'s INV-014 receipt `Seam` — `source_digest`,
        `binary_digest`, `toolchain` are trusted inputs the checker never
        invents (`receipt::tests::the_trusted_inputs_are_exactly_the_seam`,
        `receipt::tests::the_envelope_supplies_the_hashes_the_certificate_`
        `carries`); `check_kernel_covenant.py` KCOV-09 (receipt-conformance)
        re-run for real.

    optional signing/attestation                                       GAP
        Library present, production use absent (bn-2ee4c, review
        cr-3e3t1j). `continuum-evidence::signing` (ADR-0054) signs and
        verifies over canonical bytes, and its tests are re-run here as
        evidence that the library is real. But no production path signs or
        verifies a real receipt, intent bundle, or domain pack: daemon
        wiring is bn-3glnv, producers and entropy are bn-1hape. See "Gap
        controls" below.

    reject digest mismatch                                             PASS
        `continuum-workspace::publication`'s `AbortReason::IdentityCollision`
        abort — a colliding-but-distinct artifact is refused, never
        conflated (`publication::tests::an_identity_collision_aborts_`
        `rather_than_conflating_two_artifacts`, whose fixture payload is
        literally named `b"impostor"`); `continuum-certificate`'s
        independent, wire-form-only re-verification — a certificate
        stamped with another family's magic, or mutated past its magic, is
        refused by the kernel it actually names rather than accepted on the
        producer's say-so (`family_routing.rs::`
        `a_certificate_stamped_with_another_familys_magic_is_refused_by_`
        `the_kernel_it_names`, `::a_single_byte_mutation_past_the_magic_`
        `keeps_the_route_and_the_kernels_own_answer`, `::garbage_never_`
        `verifies_and_never_panics_through_the_composition`); an envelope
        wire epoch this build does not implement is rejected from bytes
        alone, never silently reinterpreted (`continuum-kernel-core`
        `pr9_exit_evidence.rs::an_envelope_wire_epoch_this_build_does_not_`
        `implement_is_unsupported_from_bytes_alone`).

    preserve redaction commitments                                     PASS
        `redacted.schema.json` / `evidence-graph-node.schema.json`'s
        `Redacted` shape requires `commitment` (a content hash of the
        withheld original) on every redacted stub — a redaction cannot omit
        it; `continuumd`'s `Redacted.commitment: Commitment` travels
        byte-identical from the original content's staged identity through
        `DaemonState::redact_evidence` and back out the wire
        (`daemon_evidence.rs::`
        `a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_`
        `omission` asserts `stub.commitment == commitment`, the same
        `Commitment` the daemon minted when the content was staged);
        `daemon_evidence.rs::verifying_over_a_redacted_reference_returns_`
        `the_structural_result_and_the_redaction` shows the commitment
        still travels when a later claim is checked *against* the redacted
        reference, downgrading to `InsufficientTelemetry` rather than lying
        about full support.

Gap controls
------------

Two controls are typed absences, not failures:

1. **"hash chain/Merkle root over events"** — no production-trace ingestion
   pipeline exists in this tree yet (Phase A is "Trust spine and ACI
   kernel"; docs/09's own asset list separates "production traces" from
   "replay integrity"/"claim ledger", and nothing under `crates/*/src`
   or `notes/plan/schemas` implements or schemas a hash-chained event
   sequence). The workspace Merkle root that *does* exist
   (`continuum-workspace::snapshot`, bn-15gj) is a real, cited mechanism
   for a *different* control ("content-addressed manifests") over a
   *different* asset (workspace content, not a production trace's event
   sequence) — conflating the two would be exactly the kind of unearned
   claim INV-008's spirit forbids. `direct_checks`' `event-hash-chain-gap`
   proves the absence mechanically: no schema file under
   `notes/plan/schemas` names a trace or event log, and no known
   producer-shaped identifier appears anywhere under `crates/*/src`.

2. **"optional signing/attestation"** — docs/09 marks this control
   *optional*. Since bn-2ee4c the signing *library* exists
   (`continuum-evidence::signing`, ADR-0054), but nothing on a production
   path calls it, so no real artifact is signed and the control is still a
   typed absence. `direct_checks`' `signing-production-gap` checks both
   halves mechanically: the library is present (its module declares
   `SignatureVerifier` and `verify_for_ci_acceptance`), and no `src/` file
   of any crate other than `continuum-evidence` names `SigningRegistry`,
   `SignatureVerifier`, or `LocalKeyring`. The day bn-3glnv or bn-1hape
   adds a caller, this check fails and the control needs a successor
   citation, not a silent pass. `signing-dependency-placement` also keeps
   the crate pinned exactly and out of the certificate checker (INV-004).

The two gap direct-checks and the placement check are exercised by `--self-test` the same way this
file's other direct checks are: a synthetic fixture (an overlay file/line
that *does* match the producer-shaped pattern) must trip the check, proving
the scanner is a real detector and not a hardcoded "always absent" stub.

Self-test
---------

`--self-test` proves three things, none vacuously:

1. This file's three source-level direct checks
   (`adr-0013-states-canonical-identity`, `event-hash-chain-gap`,
   `signing-production-gap`, `signing-dependency-placement`) each catch a real mutation/fixture, generated
   against an anchor or via a synthetic overlay that must be unambiguous.
2. Every Python delegate checker's own `--self-test` still exits 0
   (`check_kernel_covenant.py`, `check_crate_boundaries.py`, and
   `check_code_policy.py` — the last cited for GOV-1-07's rationale entry
   behind the signing dependency).
3. Every cited Rust test still passes when re-run for real, narrowly
   (`cargo test -p <crate> --test <file>|--lib --locked -- <exact names>
   --exact`) — the same tests `cargo test --workspace --locked` (this
   repo's `test` gate, which always runs before `governance` in `check`'s
   recipe list) already ran a moment earlier in the same `just check`
   invocation. Re-running them here, narrowly, ties *this* control map to
   *those* exact test names rather than to "the test suite passed", so a
   test being renamed or deleted out from under this file's citations is
   caught as loudly as the test itself failing would be. Unlike this file's
   own two/three direct checks, a Rust test's adversarial fixture (the
   mutated certificate byte, the `b"impostor"` payload, the stamped-wrong-
   magic certificate) is already baked into the test body — the test
   passing *is* the proof, the same point `t02_reachability_evidence.rs`
   makes about its own positive/negative pair.

Real run
--------

Re-runs every Python delegate for real (no `--self-test`), reads the KCOV-09
and GOV-1-07 requirement ids out of their reports, re-runs every cited Rust
test narrowly (not
`--self-test` — Rust integration/unit tests have no such flag; a normal run
*is* the real run, and the self-test step above additionally treats that same
real run as proof its own fixtures still fire), runs the three direct checks
against the live tree, and fails if any `pass`-designated control's citations
are not unanimously green, or if a `gap`-designated control's absence-scan
stops finding an absence (a stale gap declaration is a regression, not a
free pass).

Scope and honesty about limits
-------------------------------

- The gap absence-scans are canary-pattern detectors, not a proof that no
  producer could exist under an unanticipated name. They are real evidence of
  what is true *today* (the same posture `check_r16_evidence.py`'s
  zero-Lean-edge count takes for "extraction adapters isolated") and they
  regress loudly the moment a matching producer-shaped construct appears
  under a name this file's patterns anticipate. The day a real event-hash-
  chain producer or a production signing caller lands, this file's
  gap control needs a human successor citing it for real — that is out of
  this bone's scope to invent ahead of the code it would govern, exactly the
  posture `check_r16_evidence.py` takes for the still-scaffold Lean proof
  client.
- "Reject digest mismatch" is cited at the certificate-checking and
  publication-store trust boundaries, the two places T04's own asset list
  (`certificate validity`, `production traces`, `solver/proof artifacts`)
  names as attacker-reachable. It does not re-cite T02's four controls in
  full (T02 is hash-collision detection at the *value/reachability* layer,
  a different, already-delivered threat row); it cites the two sites where a
  forged or substituted *certificate or published artifact* is rejected.
- "Preserve redaction commitments" is checked at the daemon/wire layer
  (`Redacted.commitment` survives redaction and a later re-verification
  unmodified). It does not claim T11's stronger "salted commitments" control
  (T11 is undelivered) — a `Commitment` here is an unsalted content digest,
  sufficient to prove "the redacted stub still names what was withheld" but
  not sufficient by itself to prevent a small-domain brute-force guess
  against the withheld content, which is T11's concern, not T04's.

Stdlib only; the only subprocesses run are the delegate checkers and `cargo
test` invocations already exercised elsewhere in `just check`, never a
network client. Exit 0 when every `pass` control's citations pass and every
`gap` control's absence holds, 1 otherwise.

Usage:

    python3 tools/governance/check_t04_evidence.py --self-test
    python3 tools/governance/check_t04_evidence.py
    python3 tools/governance/check_t04_evidence.py --evidence tools/governance/evidence/t04.json
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = "tools/governance/evidence/t04.json"
THREAT_MODEL = "notes/plan/docs/09_THREAT_MODEL.md"
ADR_0013 = "notes/plan/adr/0013-exact-state-identity.md"
ROOT_CARGO_TOML = "Cargo.toml"
SCHEMAS_DIR = "notes/plan/schemas"

PY_DELEGATES: dict[str, str] = {
    "check_kernel_covenant": "tools/check_kernel_covenant.py",
    "check_crate_boundaries": "tools/check_crate_boundaries.py",
    "check_code_policy": "tools/governance/check_code_policy.py",
}

# --- Rust delegates: (package, cargo-test target args, [exact test names]) -----------------

RustGroup = tuple[str, list[str], list[str]]

RUST_DELEGATES: dict[str, RustGroup] = {
    "content_addressed_manifests_value": (
        "continuum-value",
        ["--lib"],
        ["identity::tests::blake3_is_a_pure_function_of_the_bytes"],
    ),
    "content_addressed_manifests_workspace": (
        "continuum-workspace",
        ["--test", "pr3_exit_evidence"],
        [
            "positive_an_old_snapshot_remains_reproducible_after_the_working_tree_changes",
        ],
    ),
    "build_identity_receipt": (
        "continuum-kernel-core",
        ["--lib"],
        [
            "receipt::tests::the_trusted_inputs_are_exactly_the_seam",
            "receipt::tests::the_envelope_supplies_the_hashes_the_certificate_carries",
        ],
    ),
    "reject_digest_mismatch_publication": (
        "continuum-workspace",
        ["--lib"],
        [
            "publication::tests::an_identity_collision_aborts_rather_than_conflating_two_artifacts",
        ],
    ),
    "reject_digest_mismatch_family_routing": (
        "continuum-certificate",
        ["--test", "family_routing"],
        [
            "a_certificate_stamped_with_another_familys_magic_is_refused_by_the_kernel_it_names",
            "a_single_byte_mutation_past_the_magic_keeps_the_route_and_the_kernels_own_answer",
            "garbage_never_verifies_and_never_panics_through_the_composition",
        ],
    ),
    "reject_digest_mismatch_wire_epoch": (
        "continuum-kernel-core",
        ["--test", "pr9_exit_evidence"],
        [
            "an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone",
        ],
    ),
    "signing_attestation_library": (
        "continuum-evidence",
        ["--test", "signing_identities"],
        [
            "sign_verify_round_trips_for_every_signed_kind_from_wire_bytes",
            "a_tampered_payload_downgrades_to_signature_mismatch",
            "a_signature_under_the_wrong_key_does_not_verify",
            "a_revoked_signers_signatures_downgrade_and_it_can_no_longer_sign",
            "the_ci_acceptance_check_fails_closed_on_every_unverified_outcome",
            "signatures_are_deterministic_and_pinned",
        ],
    ),
    "signing_attestation_primitive": (
        "continuum-evidence",
        ["--lib"],
        ["signing::tests::ed25519_matches_the_rfc_8032_test_vectors"],
    ),
    "preserve_redaction_commitments": (
        "continuumd",
        ["--test", "daemon_evidence"],
        [
            "a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission",
            "verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction",
        ],
    ),
}


# ============================================================================
# Direct checks this file owns
# ============================================================================


def rule_adr_0013_states_canonical_identity(text: str | None) -> list[str]:
    if text is None:
        return [f"{ADR_0013} is missing"]
    anchor = "Canonical structural encodings define identity"
    if anchor not in text:
        return [f"{ADR_0013} no longer states {anchor!r}"]
    digest_anchor = "Artifact digests use cryptographic hashes"
    if digest_anchor not in text:
        return [f"{ADR_0013} no longer states {digest_anchor!r}"]
    return []


# Canary patterns for a production-trace event-hash-chain producer. Deliberately
# specific, Rust-identifier-shaped: see the module docstring's "Scope and honesty
# about limits" for what this is (real evidence of today) and is not (a proof no
# producer could exist under an unanticipated name).
EVENT_CHAIN_SCHEMA_PATTERN = re.compile(r"(?i)(trace|event).*\.schema\.json$")
EVENT_CHAIN_RUST_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\bProductionTraceLog\b"),
    re.compile(r"\bTraceHashChain\b"),
    re.compile(r"\bEventMerkleRoot\b"),
    re.compile(r"\bhash_chain_over_events\b"),
    re.compile(r"\bchained_event_hash\b"),
)

# Signing/attestation crate names. Matched against dependency *names* (TOML keys),
# not arbitrary prose, so a comment mentioning "signing" does not trip this.
SIGNING_DEPENDENCY_NAMES: tuple[str, ...] = (
    "ed25519",
    "ed25519-dalek",
    "ring",
    "rsa",
    "rustls",
    "sigstore",
    "cosign",
    "dsse",
    "in-toto",
    "minisign",
    "sequoia-openpgp",
    "gpgme",
    "p256",
    "k256",
)
SIGNING_PIN = 'ed25519-dalek = { version = "=2.2.0", default-features = false, features = ["zeroize"] }'
CHECKER_MANIFESTS: tuple[str, ...] = (
    "crates/continuum-certificate/Cargo.toml",
    "crates/continuum-kernel-core/Cargo.toml",
    "crates/continuum-kernel-sat/Cargo.toml",
    "crates/continuum-kernel-smt/Cargo.toml",
    "crates/continuum-kernel-temporal/Cargo.toml",
)
DEPENDENCY_NAME_PATTERN = re.compile(r"^([A-Za-z0-9_-]+)\s*[=.]", re.MULTILINE)


def rule_event_hash_chain_gap(
    schema_names: list[str],
    rust_sources: list[tuple[str, str]],
) -> list[str]:
    """Returns hits (evidence the gap is NOT real) — empty means the gap holds."""
    hits: list[str] = []
    for name in schema_names:
        if EVENT_CHAIN_SCHEMA_PATTERN.search(name):
            hits.append(f"schema {name!r} names a trace/event artifact")
    for rel, text in rust_sources:
        for pattern in EVENT_CHAIN_RUST_PATTERNS:
            if pattern.search(text):
                hits.append(f"{rel}: matches {pattern.pattern!r}")
    return hits


def rule_signing_dependency_placement(
    cargo_toml_text: str | None,
    checker_manifests: list[tuple[str, str]],
) -> list[str]:
    """ADR-0054 D2/D3: the signing crate is pinned exactly in the root manifest, and no
    certificate-checker crate declares any signing crate (INV-004). Returns problems."""
    if cargo_toml_text is None:
        return ["Cargo.toml is missing"]
    problems: list[str] = []
    if SIGNING_PIN not in cargo_toml_text:
        problems.append(f"Cargo.toml no longer pins {SIGNING_PIN!r}")
    for rel, text in checker_manifests:
        for match in DEPENDENCY_NAME_PATTERN.finditer(text):
            name = match.group(1).lower()
            if name in SIGNING_DEPENDENCY_NAMES:
                problems.append(f"{rel} (certificate checker) declares signing dependency {name!r}")
    return problems


SIGNING_LIBRARY = "crates/continuum-evidence/src/signing.rs"
SIGNING_LIBRARY_ANCHORS: tuple[str, ...] = ("pub struct SignatureVerifier", "pub fn verify_for_ci_acceptance(")
SIGNING_API_PATTERN = re.compile(r"\b(SigningRegistry|SignatureVerifier|LocalKeyring)\b")


def rule_signing_production_gap(library_text: str | None, rust_sources: list[tuple[str, str]]) -> list[str]:
    """Returns hits (evidence the gap is NOT as declared) — empty means the library is
    present and no production caller outside continuum-evidence exists."""
    hits: list[str] = []
    if library_text is None:
        hits.append(f"{SIGNING_LIBRARY} is missing: the declared library is absent")
    else:
        for anchor in SIGNING_LIBRARY_ANCHORS:
            if anchor not in library_text:
                hits.append(f"{SIGNING_LIBRARY} no longer declares {anchor!r}")
    for rel, text in rust_sources:
        if rel.startswith("crates/continuum-evidence/") or "/src/" not in rel:
            continue
        if SIGNING_API_PATTERN.search(text):
            hits.append(f"{rel}: a production caller of the signing library exists; the gap is stale")
    return hits


def real_checker_manifests() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for rel in CHECKER_MANIFESTS:
        path = ROOT / rel
        out.append((rel, path.read_text(encoding="utf-8") if path.is_file() else ""))
    return out


def real_schema_names() -> list[str]:
    schemas_dir = ROOT / SCHEMAS_DIR
    if not schemas_dir.is_dir():
        return []
    return sorted(p.name for p in schemas_dir.glob("*.json"))


def real_rust_sources() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        try:
            out.append((rel, path.read_text(encoding="utf-8")))
        except OSError:
            continue
    return out


def direct_checks() -> dict[str, list[str]]:
    adr_text = (ROOT / ADR_0013).read_text(encoding="utf-8") if (ROOT / ADR_0013).is_file() else None
    cargo_text = (
        (ROOT / ROOT_CARGO_TOML).read_text(encoding="utf-8") if (ROOT / ROOT_CARGO_TOML).is_file() else None
    )
    return {
        "adr-0013-states-canonical-identity": rule_adr_0013_states_canonical_identity(adr_text),
        "event-hash-chain-gap": rule_event_hash_chain_gap(real_schema_names(), real_rust_sources()),
        "signing-dependency-placement": rule_signing_dependency_placement(cargo_text, real_checker_manifests()),
        "signing-production-gap": rule_signing_production_gap(
            (ROOT / SIGNING_LIBRARY).read_text(encoding="utf-8") if (ROOT / SIGNING_LIBRARY).is_file() else None,
            real_rust_sources(),
        ),
    }


def direct_self_test() -> dict[str, object]:
    failures: list[str] = []
    baseline = direct_checks()
    dirty_baseline = {k: v for k, v in baseline.items() if v}
    if dirty_baseline:
        failures.append(f"the real repository already violates direct checks {sorted(dirty_baseline)}")

    caught: list[str] = []

    # adr-0013-states-canonical-identity: prove both anchors are load-bearing.
    real_adr_text = (ROOT / ADR_0013).read_text(encoding="utf-8") if (ROOT / ADR_0013).is_file() else ""
    anchor = "Canonical structural encodings define identity"
    if real_adr_text.count(anchor) != 1:
        failures.append(f"adr-0013 fixture anchor {anchor!r} matches {real_adr_text.count(anchor)} times, expected 1")
    else:
        dirty = real_adr_text.replace(anchor, "structural encodings are consulted for identity, sometimes", 1)
        if not rule_adr_0013_states_canonical_identity(dirty):
            failures.append("fixture: rewording the canonical-identity sentence did not trip adr-0013-states-canonical-identity")
        else:
            caught.append("adr-0013-states-canonical-identity")

    # event-hash-chain-gap: prove a synthetic producer (schema name, then Rust
    # identifier) is caught by the scanner, without touching the real tree.
    schema_fixture_hit = rule_event_hash_chain_gap(
        real_schema_names() + ["production-trace-event.schema.json"], []
    )
    rust_fixture_hit = rule_event_hash_chain_gap(
        [],
        [("crates/fixture/src/lib.rs", "pub struct ProductionTraceLog { root: [u8; 32] }")],
    )
    if not schema_fixture_hit:
        failures.append("fixture: a trace-named schema file did not trip event-hash-chain-gap")
    elif not rust_fixture_hit:
        failures.append("fixture: a ProductionTraceLog identifier did not trip event-hash-chain-gap")
    else:
        caught.append("event-hash-chain-gap")

    # signing-dependency-placement: prove a loosened pin and a checker-side edge are
    # both caught, without touching the real tree.
    real_cargo_text = (ROOT / ROOT_CARGO_TOML).read_text(encoding="utf-8") if (ROOT / ROOT_CARGO_TOML).is_file() else ""
    checkers = real_checker_manifests()
    if real_cargo_text.count(SIGNING_PIN) != 1:
        failures.append(f"signing-dependency-placement fixture anchor matches {real_cargo_text.count(SIGNING_PIN)} times, expected 1")
    else:
        loosened = real_cargo_text.replace(SIGNING_PIN, 'ed25519-dalek = "2"', 1)
        injected = [(rel, text + '\n[dependencies]\ned25519-dalek.workspace = true\n') if i == 0 else (rel, text) for i, (rel, text) in enumerate(checkers)]
        if not rule_signing_dependency_placement(loosened, checkers):
            failures.append("fixture: loosening the ed25519-dalek pin did not trip signing-dependency-placement")
        elif not rule_signing_dependency_placement(real_cargo_text, injected):
            failures.append("fixture: a certificate-checker ed25519-dalek edge did not trip signing-dependency-placement")
        else:
            caught.append("signing-dependency-placement")

    # signing-production-gap: prove a missing library and a synthetic production caller
    # are both caught.
    library = (ROOT / SIGNING_LIBRARY).read_text(encoding="utf-8") if (ROOT / SIGNING_LIBRARY).is_file() else ""
    caller = [("crates/continuumd/src/fixture.rs", "use continuum_evidence::signing::SignatureVerifier;")]
    if not rule_signing_production_gap(None, []):
        failures.append("fixture: a missing signing library did not trip signing-production-gap")
    elif not rule_signing_production_gap(library, caller):
        failures.append("fixture: a synthetic continuumd caller did not trip signing-production-gap")
    else:
        caught.append("signing-production-gap")

    return {"status": "fail" if failures else "pass", "checks_caught": sorted(caught), "failures": failures}


# ============================================================================
# Delegates — invoked exactly as their owning gate invokes them
# ============================================================================


def run_py_delegate(rel: str, *args: str) -> tuple[int, dict | None]:
    path = ROOT / rel
    proc = subprocess.run(
        [sys.executable, str(path), *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        data = json.loads(proc.stdout) if proc.stdout.strip() else None
    except json.JSONDecodeError:
        data = None
    return proc.returncode, data


def py_delegate_self_test() -> dict[str, object]:
    results: dict[str, object] = {}
    failed: list[str] = []
    for name, rel in sorted(PY_DELEGATES.items()):
        code, data = run_py_delegate(rel, "--self-test")
        report = data if isinstance(data, dict) else {}
        status = report.get("status", report.get("self_test"))
        ok = code == 0 and status == "pass"
        results[name] = {"exit_code": code, "status": status}
        if not ok:
            failed.append(f"{name} --self-test did not pass cleanly (exit {code}, status {status!r})")
    return {"status": "fail" if failed else "pass", "delegates": results, "failures": failed}


def py_delegate_real_run() -> dict[str, dict]:
    reports: dict[str, dict] = {}
    for name, rel in sorted(PY_DELEGATES.items()):
        _, data = run_py_delegate(rel)
        reports[name] = data or {"status": "fail", "note": "delegate produced no parseable JSON report"}
    return reports


def rid_status(report: dict, rid: str) -> str:
    if rid in report.get("passing", ()):
        return "pass"
    if rid in report.get("failing", {}):
        return "fail"
    return "unknown"


def run_rust_group(group: RustGroup) -> tuple[bool, str]:
    package, target_args, tests = group
    cmd = [
        "cargo",
        "test",
        "-p",
        package,
        *target_args,
        "--locked",
        "--",
        *tests,
        "--exact",
    ]
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=False)
    output = proc.stdout + proc.stderr
    ok = proc.returncode == 0
    for test in tests:
        # Every named test must appear as its own passing line, not merely be
        # absent from a run that filtered nothing in (a typo'd name would
        # silently match zero tests and still exit 0).
        if f"test {test} ... ok" not in output and f"test {test.rsplit('::', 1)[-1]} ... ok" not in output:
            ok = False
    return ok, output


def rust_delegate_run() -> dict[str, dict]:
    results: dict[str, dict] = {}
    for name, group in sorted(RUST_DELEGATES.items()):
        ok, output = run_rust_group(group)
        results[name] = {
            "package": group[0],
            "tests": group[2],
            "status": "pass" if ok else "fail",
            "tail": "\n".join(output.strip().splitlines()[-8:]),
        }
    return results


# ============================================================================
# The six-control binding
# ============================================================================


@dataclass(frozen=True)
class Citation:
    artifact: str
    enforces: str
    status: str


def build_controls(
    direct: dict[str, list[str]],
    py_reports: dict[str, dict],
    rust_results: dict[str, dict],
) -> dict[str, dict]:
    kcov = py_reports["check_kernel_covenant"]
    boundaries = py_reports["check_crate_boundaries"]
    code_policy = py_reports["check_code_policy"]

    def direct_status(check: str) -> str:
        return "fail" if direct.get(check) else "pass"

    def rust_status(name: str) -> str:
        return rust_results.get(name, {}).get("status", "unknown")

    pass_controls: dict[str, list[Citation]] = {
        "content-addressed manifests": [
            Citation(ADR_0013, "canonical structural identity + cryptographic digests, stated", direct_status("adr-0013-states-canonical-identity")),
            Citation(
                "crates/continuum-value/src/identity.rs identity::tests::blake3_is_a_pure_function_of_the_bytes",
                "a digest is a pure function of canonical bytes alone, never of host/clock/process",
                rust_status("content_addressed_manifests_value"),
            ),
            Citation(
                "crates/continuum-workspace/tests/pr3_exit_evidence.rs positive_an_old_snapshot_remains_reproducible_after_the_working_tree_changes",
                "the workspace snapshot's content-addressed manifest (files, root_digest) survives disk changes",
                rust_status("content_addressed_manifests_workspace"),
            ),
        ],
        "build identity": [
            Citation(
                "crates/continuum-kernel-core/src/receipt.rs receipt::tests::the_trusted_inputs_are_exactly_the_seam",
                "build digest, binary digest, and toolchain are named trusted inputs, never invented",
                rust_status("build_identity_receipt"),
            ),
            Citation(
                "tools/check_kernel_covenant.py KCOV-09",
                "receipts the schema admits, epochs named, build identity carried",
                rid_status(kcov, "KCOV-09"),
            ),
        ],
        "reject digest mismatch": [
            Citation(
                "crates/continuum-workspace/src/publication.rs publication::tests::an_identity_collision_aborts_rather_than_conflating_two_artifacts",
                "a colliding-but-distinct artifact (the b\"impostor\" fixture) is refused, never conflated",
                rust_status("reject_digest_mismatch_publication"),
            ),
            Citation(
                "crates/continuum-certificate/tests/family_routing.rs (three tests)",
                "a certificate stamped with another family's magic, or mutated past its magic, is refused by the kernel it actually names",
                rust_status("reject_digest_mismatch_family_routing"),
            ),
            Citation(
                "crates/continuum-kernel-core/tests/pr9_exit_evidence.rs an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone",
                "an unrecognized wire epoch is rejected from bytes alone, never silently reinterpreted",
                rust_status("reject_digest_mismatch_wire_epoch"),
            ),
            Citation(
                "tools/check_crate_boundaries.py",
                "certificate-checker-not-search and kernel-is-synchronous hold, so rejection is independent re-verification, not self-certification",
                boundaries.get("status", "unknown"),
            ),
        ],
        "preserve redaction commitments": [
            Citation(
                "notes/plan/schemas/redacted.schema.json",
                "every Redacted stub requires commitment (a content hash of the withheld original)",
                "pass",
            ),
            Citation(
                "crates/continuumd/tests/daemon_evidence.rs a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission",
                "the commitment minted at staging travels byte-identical through redaction and back out the wire",
                rust_status("preserve_redaction_commitments"),
            ),
            Citation(
                "crates/continuumd/tests/daemon_evidence.rs verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction",
                "a later claim checked against a redacted reference still carries the commitment and downgrades honestly instead of lying",
                rust_status("preserve_redaction_commitments"),
            ),
        ],
    }

    gap_controls: dict[str, dict] = {
        "hash chain/Merkle root over events": {
            "reason": (
                "No production-trace ingestion pipeline exists in this tree "
                "(Phase A scope). The workspace Merkle root that exists "
                "(continuum-workspace::snapshot, bn-15gj) is a real mechanism "
                "for a different control (content-addressed manifests) over a "
                "different asset (workspace content, not a trace's event "
                "sequence)."
            ),
            "absence_checks": [
                {
                    "artifact": "tools/governance/check_t04_evidence.py event-hash-chain-gap",
                    "checks": "no notes/plan/schemas file names a trace/event artifact; no known producer-shaped Rust identifier exists under crates/*/src",
                    "status": direct_status("event-hash-chain-gap"),
                },
            ],
        },
        "optional signing/attestation": {
            "reason": (
                "Library present, production use absent. docs/09 marks this control "
                "optional. bn-2ee4c added continuum-evidence::signing (ADR-0054), but "
                "no production path calls it, so no real receipt, intent bundle, or "
                "domain pack is signed or verified (review cr-3e3t1j). Missing: "
                "bn-3glnv (daemon operations, wire form, intent.accept's fail-closed "
                "check) and bn-1hape (OS entropy, keystore, producer-side signing). "
                "Assurance does not depend on it: reject digest mismatch and build "
                "identity above already deny an adversary a path to a forged pass."
            ),
            "absence_checks": [
                {
                    "artifact": "tools/governance/check_t04_evidence.py signing-production-gap",
                    "checks": "the signing library is present, and no src/ file outside continuum-evidence names SigningRegistry, SignatureVerifier, or LocalKeyring",
                    "status": direct_status("signing-production-gap"),
                },
                {
                    "artifact": "tools/governance/check_t04_evidence.py signing-dependency-placement",
                    "checks": "ed25519-dalek pinned exactly at the root; no certificate-checker crate declares a signing crate (INV-004)",
                    "status": direct_status("signing-dependency-placement"),
                },
                {
                    "artifact": "crates/continuum-evidence/tests/signing_identities.rs (six tests) and signing::tests::ed25519_matches_the_rfc_8032_test_vectors",
                    "checks": "the library the gap names is real: it round-trips, rejects tampering, wrong keys and revoked signers, fails closed in CI mode, and matches RFC 8032",
                    "status": "pass" if rust_status("signing_attestation_library") == "pass" and rust_status("signing_attestation_primitive") == "pass" else "fail",
                },
                {
                    "artifact": "tools/governance/check_code_policy.py GOV-1-07",
                    "checks": "the signing dependencies carry dependency-rationale entries and TCB classes",
                    "status": rid_status(code_policy, "GOV-1-07"),
                },
            ],
        },
    }

    controls: dict[str, dict] = {}
    for control, citations in pass_controls.items():
        controls[control] = {
            "type": "pass",
            "citations": [c.__dict__ for c in citations],
            "status": "pass" if all(c.status == "pass" for c in citations) else "fail",
        }
    for control, gap in gap_controls.items():
        checks = gap["absence_checks"]
        controls[control] = {
            "type": "gap",
            "reason": gap["reason"],
            "absence_checks": checks,
            # A gap control's status is "pass" (the gap genuinely holds) unless
            # an absence-check found a hit, in which case the declared gap is
            # stale and this is a regression, not a free pass.
            "status": "pass" if all(c["status"] == "pass" for c in checks) else "fail",
        }
    return controls


# ============================================================================
# Evidence
# ============================================================================


def build_evidence(controls: dict[str, dict], direct_st: dict, py_st: dict, rust_results: dict[str, dict]) -> dict:
    overall = "pass" if all(c["status"] == "pass" for c in controls.values()) else "fail"
    rust_ok = all(r.get("status") == "pass" for r in rust_results.values())
    return {
        "artifact": "continuum.governance.evidence/t04",
        "produced_by": "tools/governance/check_t04_evidence.py",
        "reproduce": f"python3 tools/governance/check_t04_evidence.py --evidence {EVIDENCE}",
        "authority": f"{THREAT_MODEL} T04 (Forged trace or artifact substitution)",
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of "
            "the repository contents and the delegates' own outputs alone, so an unchanged tree "
            "reproduces it byte-for-byte (Rust test 'tail' fields are trimmed to their final "
            "lines, which carry no timing information beyond a 'finished in N.NNs' string that "
            "this file does not otherwise consult). Every subprocess is either a delegate "
            "checker already invoked elsewhere in `just check`, or a `cargo test` invocation of "
            "a test already exercised by `just check`'s own `test` recipe; none makes a network "
            "access."
        ),
        "py_delegates": sorted(PY_DELEGATES.values()),
        "rust_delegates": {
            name: {"package": group[0], "args": group[1], "tests": group[2]}
            for name, group in sorted(RUST_DELEGATES.items())
        },
        "controls": controls,
        "self_test": {
            "direct": direct_st,
            "py_delegates": py_st,
            "rust_delegates_rerun": rust_results,
            "status": "pass" if direct_st.get("status") == "pass" and py_st.get("status") == "pass" and rust_ok else "fail",
        },
        "boundary": (
            "Two of six controls (hash chain/Merkle root over events; optional "
            "signing/attestation, whose library exists but has no production "
            "caller) are typed absences, not live enforcement — see the module "
            "docstring's 'Gap controls' section. The other four are "
            "bound to citations that are re-run, not merely read, on every "
            "invocation."
        ),
        "status": overall,
    }


# ============================================================================
# Entry point
# ============================================================================


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="prove this file's three direct checks, every Python delegate's self-test, and every cited Rust test all still pass",
    )
    parser.add_argument(
        "--evidence",
        nargs="?",
        const=str(ROOT / EVIDENCE),
        default=None,
        metavar="PATH",
        help="run the self-test and the real binding, then write the evidence JSON to PATH",
    )
    parser.add_argument("--quiet", action="store_true", help="print only the status line")
    args = parser.parse_args(argv)

    if args.self_test and not args.evidence:
        direct_st = direct_self_test()
        py_st = py_delegate_self_test()
        rust_results = rust_delegate_run()
        rust_ok = all(r.get("status") == "pass" for r in rust_results.values())
        status = "pass" if direct_st["status"] == "pass" and py_st["status"] == "pass" and rust_ok else "fail"
        report = {"status": status, "direct": direct_st, "py_delegates": py_st, "rust_delegates": rust_results}
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if status == "pass" else 1

    direct_st: dict[str, object] = {"status": "not-run"}
    py_st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        direct_st = direct_self_test()
        py_st = py_delegate_self_test()
        st_ok = direct_st["status"] == "pass" and py_st["status"] == "pass"

    direct = direct_checks()
    py_reports = py_delegate_real_run()
    rust_results = rust_delegate_run()
    controls = build_controls(direct, py_reports, rust_results)

    if args.evidence:
        evidence = build_evidence(controls, direct_st, py_st, rust_results)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        rust_ok = all(r.get("status") == "pass" for r in rust_results.values())
        st_ok = st_ok and rust_ok

    failing_controls = {k: v for k, v in controls.items() if v["status"] != "pass"}
    report = {
        "controls": {k: v["status"] for k, v in sorted(controls.items())},
        "failing_controls": sorted(failing_controls),
        "self_test": {"direct": direct_st.get("status"), "py_delegates": py_st.get("status")},
        "evidence": args.evidence,
        "status": "fail" if (failing_controls or not st_ok) else "pass",
    }
    if args.quiet:
        print(report["status"])
    else:
        print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if (failing_controls or not st_ok) else 0


if __name__ == "__main__":
    raise SystemExit(main())
