# GOV §2 — executable ADR decision-process checks

Bones: `bn-29gc` (GOV-2-01..06), `bn-xjz8` (GOV-2-13..14). Both are implemented
by one script, `check_adr_process.py`, because docs/12 §2's two rules — the
closed status set and the nine required sections — are both properties of the
same scan of `notes/plan/adr/*.md`, so splitting the harness across Bones
would mean parsing every ADR twice with two sources of truth for the same
headings. `GOV-2-07`..`GOV-2-12` (also nine-section obligations) are tracked
by sibling Bone `bn-gln1`; this script already enforces and evidences all
fourteen ids, keyed individually in `evidence/gov-2.json`.

## What it checks

Source: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §2.

1. **`GOV-2-01`..`GOV-2-05`** — an ADR's status is a member of the closed set
   `{proposed, accepted, superseded, rejected, experimental}`. The status is
   read from either house style found in the real corpus (an inline
   `**Status:** X` line, or a `## Status` heading whose first line is the
   value), then normalized to its leading word (so "Accepted as target
   architecture" still counts as `accepted`). An ADR with no status field at
   all is always a failure — there's nothing to check against the set.
2. **`GOV-2-06`..`GOV-2-14`** — an ADR includes all nine required sections:
   context, decision, formal consequences, alternatives, compatibility,
   security, performance hypothesis, validation plan, rollback. Matched
   against `##`-level headings only (a nested `###` subsection never counts)
   with a tolerant-but-honest rule per section — see the module docstring in
   `check_adr_process.py` for the exact regex and the real-corpus evidence
   that motivated each one (e.g. `formal_consequences` accepts a plain
   "Consequences" heading because that's the house style everywhere; no real
   ADR spells out "Formal consequences").

## The grandfather list

As scanned 2026-07-31, **52 of the 53 real ADRs** are missing at least one of
the nine required sections — only `ADR-0053` is fully compliant. Section
discipline tightened over the corpus's life: `compatibility` / `security` /
`performance_hypothesis` appear only from ADR-0029/0053 on, and the earliest
20-odd ADRs also predate a dedicated `alternatives` heading style.

`adr-grandfathered.toml` names, per pre-existing ADR file, exactly which
required sections it lacks. `check_adr_process.py`:

- exempts only the `(adr, section)` pairs actually listed;
- still fails a grandfathered ADR for any other gap, or for a status outside
  the closed set (status is never grandfathered);
- fails the run outright if an entry names a section that's actually present
  (a stale exemption) or names an ADR file that doesn't exist;
- gives zero exemption to any ADR not listed — new ADRs must be fully
  compliant from the day they land.

Rerun the real scan after adding or editing an ADR; entries should shrink
over time as ADRs are brought up to the nine-section standard, never grow.

## Usage

```sh
# Run every violating fixture; fails if any goes uncaught.
python3 tools/governance/check_adr_process.py --self-test

# Scan the real corpus (applies the grandfather allowlist).
python3 tools/governance/check_adr_process.py

# Run both, and (re)write evidence/gov-2.json keyed by GOV-2-01..14.
python3 tools/governance/check_adr_process.py --evidence
```

Fixtures live under `fixtures/adr/`: one out-of-set status, one ADR with no
status at all, and nine ADRs each missing exactly one required section (so
every one of `GOV-2-06`..`GOV-2-14` has its own dedicated violating fixture).
None of the fixtures are grandfathered — `--self-test` parses them directly.

## Not wired into `just check`

Per the workspace fence for bn-29gc/bn-xjz8, this Bone does not touch the
Justfile — the lead wires the gate after the wave lands. Until then, run the
two commands above directly, or add:

```just
adr-process:
    python3 tools/governance/check_adr_process.py --self-test
    python3 tools/governance/check_adr_process.py
```
