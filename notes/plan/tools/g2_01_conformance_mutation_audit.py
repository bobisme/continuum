#!/usr/bin/env python3
"""G2-01 acceptance: mutate the client, then ask the conformance checkers.

`crates/continuumd/tests/idl_conformance.rs` already carries a non-vacuity
self-test (`the_check_is_not_vacuous`, `a_removed_operation_is_reported`). That
test mutates the **IDL text in memory** and asserts the comparison reports it.
This harness is deliberately the other direction and a different method: it
mutates the **shipped transcription and the shipped client on disk** and then
runs each conformance checker as its own process, so the answer is "which
checker, running for real, reported this drift" rather than "the comparison
function is not vacuous".

Three outcomes are distinguished per checker, and the third is not a footnote:

- ``caught``   the checker ran and failed;
- ``blind``    the checker ran and passed with the mutant in place;
- ``compile``  the crate did not build, so the *compiler* rejected the drift
               and the checker never got to speak. Load-bearing for the G2-01
               reading: a divergence class that is unspellable is bound by the
               type system, not by a conformance check, and the two are
               different guarantees with different blast radii.

`continuum-mcp/tests/typed_surface.rs` is run as a fourth column. It is **not**
a conformance checker — it is the agent client's own behavioural suite, and it
is here to show what does catch client drift when neither IDL checker can.

Usage
-----

    python3 notes/plan/tools/g2_01_conformance_mutation_audit.py
    python3 notes/plan/tools/g2_01_conformance_mutation_audit.py --json
    python3 notes/plan/tools/g2_01_conformance_mutation_audit.py --only field-renamed

The matrix this recorded, at IDL 1.11 / protocol 3.6 (bn-2wypi)
--------------------------------------------------------------

::

    mutant                           idl-conf    reg-agree        g2-01   typed-surf    benchmark
    field-renamed                      caught        blind        blind        blind        blind
    field-type-changed                 caught        blind        blind        blind      compile
    field-presence-changed             caught        blind        blind        blind      compile
    operation-dropped                  caught       caught       caught        blind        blind
    authority-widened                  caught       caught        blind        blind        blind
    annotation-dropped                 caught       caught        blind        blind        blind
    error-code-dropped                 caught        blind        blind        blind        blind
    verdict-changed                    caught       caught        blind        blind        blind
    since-staled                        blind        blind        blind        blind        blind
    client-grammar-misspelled           blind        blind       caught       caught       caught
    client-calls-wrong-operation        blind        blind       caught        blind        blind

Read three ways.

- **Leg one is the whole of the structural binding.** Every drift between the
  IDL and the transcribed types is reported by `idl_conformance.rs` and by
  nothing else. Leg two (`registry_agreement.rs`) is a genuine independent
  authority for the *registry row* — operation set, authority, annotations,
  verdict family — and is blind to every field-level fact: a renamed field, a
  changed type, a changed presence marker and a dropped error code all pass it.
  That is by construction, not by defect: RFC 0027's table has no field column.
  The consequence is that field-level conformance rests on **one** checker.
- **`@since` is untranscribed, so no checker can hold it.** The IDL carries 14
  declaration-level `@since` sites; nothing on the shipped side has anywhere to
  put one, and leg one filters the annotation out by name. A version stamp that
  says a 3.6 operation arrived at 1.0 is accepted by everything in the tree.
  *Superseded by bn-7xz8v:* `OperationSpec::since` and `protocol::since` now
  transcribe every date, the daemon's version gate reads them, and leg one
  compares all of them with the IDL, so `since-staled` is caught by `idl-conf`
  by construction. The matrix above is the bn-2wypi record and was not re-run.
- **Neither conformance checker looks at a shipped client.** Both compare the
  *transcription*. The agent client's grammar naming an operation the protocol
  does not declare, and `AgentClient::task_cancel` putting `task.status` on the
  wire, are both invisible to them. The second is invisible to the client's own
  behavioural suite too — `task_cancel` is never called in `typed_surface.rs` —
  and, as the matrix records, to the benchmark suite that does call it: with the
  mutant in place both suites still pass.
  `crates/continuumd/tests/gate_g2_01_acceptance.rs` is where that binding is
  made, and where it was made for the first time.

Safety
------

Every mutant is applied to the working tree and reverted in a ``finally``
block, and the harness refuses to start unless every file it will touch is
clean in git. Originals are also written to a temporary directory whose path is
printed, so a hard kill is recoverable by hand. The final act of every run is a
digest check that every touched file is byte-identical to how it started.

Stdlib only. Nothing here is wired into the Justfile: it is an audit, run on
demand, not a gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

# The two independent conformance checkers, then three reference columns. Order is
# the reporting order.
CHECKERS: dict[str, list[str]] = {
    "idl-conf": [
        "cargo", "test", "--locked", "-p", "continuumd", "--test", "idl_conformance",
    ],
    "reg-agree": [
        "cargo", "test", "--locked", "-p", "continuumd", "--test", "registry_agreement",
    ],
    "g2-01": [
        "cargo", "test", "--locked", "-p", "continuumd", "--test", "gate_g2_01_acceptance",
    ],
    "typed-surf": [
        "cargo", "test", "--locked", "-p", "continuum-mcp", "--test", "typed_surface",
    ],
    "benchmark": ["cargo", "test", "--locked", "-p", "continuum-benchmark"],
}

ROLES: dict[str, str] = {
    "idl-conf": "conformance checker: the transcribed types against the IDL, own parser",
    "reg-agree": "conformance checker: the same types against RFC 0027/0026 and continuum-value",
    "g2-01": "this bone's acceptance file (crates/continuumd/tests/gate_g2_01_acceptance.rs)",
    "typed-surf": "the agent client's own behavioural suite — not a conformance checker",
    "benchmark": "the suite that drives the agent client end to end — not a conformance checker",
}

CONFORMANCE_CHECKERS = ("idl-conf", "reg-agree")


@dataclass(frozen=True)
class Mutant:
    """One representative divergence between a shipped artifact and the IDL."""

    ident: str
    divergence_class: str
    side: str
    path: str
    anchor: str
    replacement: str
    note: str


# The anchors are multi-line where a single line is not unique. A mutant whose
# anchor does not occur exactly once is a harness error, not a finding, and the
# run aborts on it.
MUTANTS: tuple[Mutant, ...] = (
    Mutant(
        ident="field-renamed",
        divergence_class="renamed field",
        side="transcription",
        path="crates/continuumd/src/protocol/operations/debug.rs",
        anchor=(
            "    struct DebugEnabledRequest {\n"
            "        /// IDL `branch: DebugHandle required`.\n"
            "        branch: DebugHandle required;"
        ),
        replacement=(
            "    struct DebugEnabledRequest {\n"
            "        /// IDL `branch: DebugHandle required`.\n"
            "        subject: DebugHandle required;"
        ),
        note="the request field the IDL calls `branch` is transcribed as `subject`",
    ),
    Mutant(
        ident="field-type-changed",
        divergence_class="wrong type",
        side="transcription",
        path="crates/continuumd/src/protocol/operations/debug.rs",
        anchor=(
            "    struct DebugStateRequest {\n"
            "        /// IDL `branch: DebugHandle required`.\n"
            "        branch: DebugHandle required;"
        ),
        replacement=(
            "    struct DebugStateRequest {\n"
            "        /// IDL `branch: DebugHandle required`.\n"
            "        branch: ArtifactHandle required;"
        ),
        note="a `DebugHandle` field is transcribed as the wider `ArtifactHandle`",
    ),
    Mutant(
        ident="field-presence-changed",
        divergence_class="wrong presence",
        side="transcription",
        path="crates/continuumd/src/protocol/operations/debug.rs",
        anchor=(
            "    struct DebugOpenRequest {\n"
            "        /// IDL `subject: ArtifactHandle required`.\n"
            "        subject: ArtifactHandle required;\n"
            "        /// IDL `observer: String optional`.\n"
            "        observer: String optional;"
        ),
        replacement=(
            "    struct DebugOpenRequest {\n"
            "        /// IDL `subject: ArtifactHandle required`.\n"
            "        subject: ArtifactHandle required;\n"
            "        /// IDL `observer: String optional`.\n"
            "        observer: String nullable;"
        ),
        note="`optional` (may be absent) transcribed as `nullable` (present, may be null)",
    ),
    Mutant(
        ident="operation-dropped",
        divergence_class="missing operation",
        side="transcription",
        path="crates/continuumd/src/protocol/registry.rs",
        anchor=(
            "    OperationSpec {\n"
            '        name: "debug.enabled",\n'
            "        authority: AuthorityLevel::Execute,\n"
            "        since: None,\n"
            "        annotations: &[Annotation::Readonly, Annotation::Paginated],\n"
            "        request: StructSpec::of::<DebugEnabledRequest>(),\n"
            "        response: StructSpec::of::<DebugEnabledResponse>(),\n"
            "        verdict: None,\n"
            "        events: None,\n"
            "        errors: &[ErrorCode::UnsupportedSemanticFeature],\n"
            "    },\n"
        ),
        replacement="",
        note="one of the IDL's 75 operations is absent from the shipped registry",
    ),
    Mutant(
        ident="authority-widened",
        divergence_class="wrong authority level",
        side="transcription",
        path="crates/continuumd/src/protocol/registry.rs",
        anchor=(
            '        name: "intent.lock",\n'
            "        authority: AuthorityLevel::ReviseIntent,"
        ),
        replacement=(
            '        name: "intent.lock",\n'
            "        authority: AuthorityLevel::Promote,"
        ),
        note="a privileged operation is transcribed at the wrong authority level",
    ),
    Mutant(
        ident="annotation-dropped",
        divergence_class="dropped annotation",
        side="transcription",
        path="crates/continuumd/src/protocol/registry.rs",
        anchor=(
            '        name: "task.subscribe",\n'
            "        authority: AuthorityLevel::Read,\n"
            "        since: None,\n"
            "        annotations: &[Annotation::Readonly, Annotation::Streaming],"
        ),
        replacement=(
            '        name: "task.subscribe",\n'
            "        authority: AuthorityLevel::Read,\n"
            "        since: None,\n"
            "        annotations: &[Annotation::Streaming],"
        ),
        note="`@readonly` is dropped, so the idempotency-key obligation flips",
    ),
    Mutant(
        ident="error-code-dropped",
        divergence_class="dropped error code",
        side="transcription",
        path="crates/continuumd/src/protocol/registry.rs",
        anchor=(
            "        errors: &[\n"
            "            ErrorCode::IntentMutationDenied,\n"
            "            ErrorCode::AcceptanceChainInvalid,\n"
            "            ErrorCode::PolicyGateFailed,\n"
            "        ],"
        ),
        replacement=(
            "        errors: &[\n"
            "            ErrorCode::IntentMutationDenied,\n"
            "            ErrorCode::PolicyGateFailed,\n"
            "        ],"
        ),
        note="an operation's declared error clause loses a code",
    ),
    Mutant(
        ident="verdict-changed",
        divergence_class="wrong verdict type",
        side="transcription",
        path="crates/continuumd/src/protocol/registry.rs",
        anchor=(
            '        name: "workspace.create",\n'
            "        authority: AuthorityLevel::Propose,\n"
            "        since: None,\n"
            "        annotations: &[Annotation::Mutation],\n"
            "        request: StructSpec::of::<WorkspaceCreateRequest>(),\n"
            "        response: StructSpec::of::<WorkspaceCreateResponse>(),\n"
            '        verdict: Some("StructuralVerdictValue"),'
        ),
        replacement=(
            '        name: "workspace.create",\n'
            "        authority: AuthorityLevel::Propose,\n"
            "        since: None,\n"
            "        annotations: &[Annotation::Mutation],\n"
            "        request: StructSpec::of::<WorkspaceCreateRequest>(),\n"
            "        response: StructSpec::of::<WorkspaceCreateResponse>(),\n"
            '        verdict: Some("PolicyVerdictValue"),'
        ),
        note="the verdict family an operation answers in is transcribed wrongly",
    ),
    Mutant(
        ident="since-staled",
        divergence_class="stale @since version",
        side="idl",
        path="notes/plan/schemas/continuumd-native-protocol.idl",
        anchor='@since("3.6")',
        replacement='@since("1.0")',
        note="the IDL dates protocol 3.6's newest operation to a version that never existed",
    ),
    Mutant(
        ident="client-grammar-misspelled",
        divergence_class="client names a non-existent operation",
        side="agent client",
        path="crates/continuum-mcp/src/register.rs",
        anchor='        operation: "workspace.seal",',
        replacement='        operation: "workspace.sealed",',
        note="the shipped action grammar names an operation the protocol does not declare",
    ),
    Mutant(
        ident="client-calls-wrong-operation",
        divergence_class="client method calls the wrong operation",
        side="agent client",
        path="crates/continuum-mcp/src/client.rs",
        anchor="        let arguments = Arguments::TaskCancel(TaskCancelRequest { task: task.clone() });",
        replacement="        let arguments = Arguments::TaskStatus(TaskStatusRequest { task: task.clone() });",
        note="`AgentClient::task_cancel` puts `task.status` on the wire",
    ),
)


def digest(path: Path) -> str:
    """The sha256 of a file, so a revert can be proved rather than assumed."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_clean(paths: set[str]) -> None:
    """Refuse to run unless every file this harness will edit is clean in git."""
    dirty = subprocess.run(
        ["git", "status", "--porcelain", "--", *sorted(paths)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    if dirty:
        raise SystemExit(
            "refusing to mutate a dirty tree; commit or stash first:\n" + dirty
        )


def run_checker(command: list[str]) -> tuple[str, str]:
    """Run one checker, returning its outcome and the first line that explains it."""
    finished = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True, check=False
    )
    combined = finished.stdout + finished.stderr
    if "error[E" in combined or "could not compile" in combined:
        reason = next(
            (line.strip() for line in combined.splitlines() if line.startswith("error")),
            "compilation failed",
        )
        return "compile", reason
    if finished.returncode != 0:
        reason = next(
            (
                line.strip()
                for line in combined.splitlines()
                if line.startswith("test ") and " FAILED" in line
            ),
            "test binary reported failure",
        )
        return "caught", reason
    return "blind", "passed with the mutant in place"


def apply_mutant(mutant: Mutant, backups: Path) -> dict[str, object]:
    """Apply one mutant, run every checker, revert, and prove the revert."""
    target = ROOT / mutant.path
    original = target.read_text(encoding="utf-8")
    before = hashlib.sha256(original.encode("utf-8")).hexdigest()
    occurrences = original.count(mutant.anchor)
    if occurrences != 1:
        raise SystemExit(
            f"{mutant.ident}: anchor occurs {occurrences} times in {mutant.path}; "
            "an anchor that is not unique is a harness defect, not a finding"
        )
    (backups / mutant.ident).write_text(original, encoding="utf-8")

    results: dict[str, object] = {}
    try:
        target.write_text(
            original.replace(mutant.anchor, mutant.replacement), encoding="utf-8"
        )
        for name, command in CHECKERS.items():
            outcome, reason = run_checker(command)
            results[name] = {"outcome": outcome, "reason": reason}
    finally:
        target.write_text(original, encoding="utf-8")

    after = digest(target)
    if after != before:
        raise SystemExit(
            f"{mutant.ident}: {mutant.path} was not restored ({before} -> {after}); "
            f"the original is at {backups / mutant.ident}"
        )
    return results


def main() -> int:
    """Run the audit."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit the matrix as JSON")
    parser.add_argument(
        "--only", action="append", default=[], help="run only the named mutant(s)"
    )
    arguments = parser.parse_args()

    selected = [m for m in MUTANTS if not arguments.only or m.ident in arguments.only]
    if not selected:
        raise SystemExit(f"no mutant matches {arguments.only}")
    require_clean({m.path for m in selected})

    backups = Path(tempfile.mkdtemp(prefix="g2-01-mutants-"))
    print(f"# originals backed up under {backups}", file=sys.stderr)

    baseline: dict[str, object] = {}
    for name, command in CHECKERS.items():
        outcome, reason = run_checker(command)
        baseline[name] = {"outcome": outcome, "reason": reason}
        if outcome != "blind":
            raise SystemExit(
                f"baseline {name} does not pass ({outcome}: {reason}); "
                "a mutation audit over a red baseline measures nothing"
            )

    report: list[dict[str, object]] = []
    for mutant in selected:
        results = apply_mutant(mutant, backups)
        conformance = [
            results[name]["outcome"]  # type: ignore[index]
            for name in CONFORMANCE_CHECKERS
        ]
        report.append(
            {
                "mutant": mutant.ident,
                "class": mutant.divergence_class,
                "side": mutant.side,
                "path": mutant.path,
                "note": mutant.note,
                "checkers": results,
                "caught_by_a_conformance_checker": "caught" in conformance,
            }
        )
        print(f"# {mutant.ident}: {conformance}", file=sys.stderr)

    shutil.rmtree(backups, ignore_errors=True)

    if arguments.json:
        print(json.dumps({"baseline": baseline, "mutants": report}, indent=2))
        return 0

    width = max(len(m.ident) for m in selected)
    columns = list(CHECKERS)
    for name in columns:
        print(f"# {name:<11} {ROLES[name]}")
    print()
    print(f"{'mutant'.ljust(width)}  " + "  ".join(f"{c:>11}" for c in columns))
    for row in report:
        cells = "  ".join(
            f"{row['checkers'][c]['outcome']:>11}"  # type: ignore[index]
            for c in columns
        )
        print(f"{str(row['mutant']).ljust(width)}  {cells}")
    print()
    for row in report:
        if not row["caught_by_a_conformance_checker"]:
            print(f"BLIND SPOT  {row['mutant']}: {row['note']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
