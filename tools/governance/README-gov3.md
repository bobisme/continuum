# GOV §3 — claim governance, made executable

`check_claim_governance.py` is the CI check for
[`docs/12_GOVERNANCE_AND_ENGINEERING.md`](../../notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md)
§3: seven controlled claim verbs in public documents, and "CI cross-checks claim
IDs."

```sh
python3 tools/governance/check_claim_governance.py --self-test   # fixtures first
python3 tools/governance/check_claim_governance.py               # the real corpus
python3 tools/governance/check_claim_governance.py --self-test --evidence
```

The third form writes [`evidence/gov-3.json`](evidence/gov-3.json), keyed by
`GOV-3-01` … `GOV-3-07`. Exit 0 when every rule holds, 1 otherwise. Python 3
stdlib only, no network, no cargo.

## The claim registry was already there

Nothing here invents a claim registry. Three artifacts already exist and are
cross-checked against each other:

| Artifact | Role |
|---|---|
| `notes/plan/docs/18_CLAIMS_MATRIX.md` | the registry: 35 claim rows `C001`–`C035`, the closed evidence-state list, the state→wording table, and the rule rejecting bare "verified" |
| `notes/plan/notes/PLAN_REQUIREMENTS.json` (`category: claim`) | the generated mirror the traceability and label contract are computed from |
| `.bones/events/*.events` | the ownership ledger: which Bone owns which claim |

`docs/18` is authoritative; the mirror is generated from it; the ledger says who
owns each row. The cross-check binds all three so none can drift alone.

## The requirement IDs

`PLAN_REQUIREMENTS.json` gives `GOV-3-01` … `GOV-3-07` one controlled verb each:

| ID | Verb | ID | Verb |
|---|---|---|---|
| GOV-3-01 | `observed` | GOV-3-05 | `proved under` |
| GOV-3-02 | `tested` | GOV-3-06 | `certificate checked` |
| GOV-3-03 | `bounded` | GOV-3-07 | `hypothesized` |
| GOV-3-04 | `exhaustively checked` | | |

§3's closing sentence, "CI cross-checks claim IDs.", carries **no requirement ID
of its own**. Its rules are therefore recorded against all seven IDs and
collected under `cross_check` in the evidence file. That is the bn-18cg half of
this work.

## Rules

| Rule | What it enforces | Obligation |
|---|---|---|
| `claim-verb-registered` | §3 lists exactly the seven verbs, in order, and each `GOV-3-0N` entry names verb *N* | one per ID |
| `claim-verb-discipline` | a controlled verb used in a claim carries its discipline: `observed` names a scope, `tested` a corpus, `bounded` / `exhaustively checked` a bound, `proved under` its assumptions, `certificate checked` its artifact, `hypothesized` no stronger co-claim | one per ID |
| `claim-uncontrolled-verb` | a verification outcome asserted with a verb outside the seven — bare "verified", "guaranteed", "proven", "sound", "complete", "deterministic", "production-equivalent" (docs/18's own list, plus the two the bone names) | the closed-set half |
| `claim-id-resolves` | every claim ID cited in a scanned document names a real registry row | cross-check |
| `claim-registry-mirror` | registry ↔ generated mirror, both directions, row for row, text and source line | cross-check |
| `claim-registered-referenced` | every *active* registered claim is referenced at least once outside the registry — by a scanned document or by a Bone | cross-check |
| `claim-state-declared` | every row's evidence state is one the registry declares, and the declared list itself has not drifted | cross-check |

## Scan scope, chosen deliberately

Scanned (58 documents): `README.md`, `notes/plan/README.md`,
`notes/plan/docs/*.md`, `notes/plan/plan.md` — the public, claim-bearing
surface.

Not scanned: RFCs, ADRs, `research/`, `plan.review.*.md`, `archive/`, `spikes/`,
`corpus/`, `crates/`, `AGENTS.md`. RFCs and ADRs are specifications and review
transcripts are arguments; neither is where a public claim is published. **A
claim smuggled into an RFC is outside this check.**

Removed inside every scanned document: fenced code blocks, inline code spans,
HTML comments, and curly-quoted strings. These carry CLI renderings, schema
text, and vocabulary citations, not assertions — it is how §3's own verb list
and docs/18's wording table are *defined* here rather than *scanned* here. Hard
wrapping is undone before sentences are split, so a claim broken across source
lines is still read as one claim.

## Boundaries

The two prose rules are proxies, and the evidence file records that per entry.

- The claim-assertion grammar matches copula/perfect constructions with the verb
  in predicate position. A claim written as a nominalization ("verification of X
  is complete work"), a bare table cell, or a figure caption is not seen.
- Exemptions are sentence-scoped: a claim sharing a sentence with a negation, a
  modal, or a subordinating conjunction is treated as a criterion rather than a
  claim. A bare claim can therefore hide next to an unrelated "must".
- `claim-verb-discipline` checks that a scope, bound, assumption, or artifact is
  *named*, never that it is the right one. Nothing here re-checks a certificate.
- `claim-registered-referenced` accepts a Bone that owns the claim, so it proves
  ownership, not published citation. Today 32 of 33 active claims are referenced
  only by their Bones; `C035` is the one cited in a scanned document.
- Verbs with zero assertive uses in scope (`observed`, `exhaustively checked`,
  `proved under`, `certificate checked`, `hypothesized` as of this writing) have
  their discipline rule exercised by fixture only. Each such entry says so in
  `real_run.note`; `claim-verb-registered` and the cross-check rules still bind
  on real content for those IDs.

## Grandfathered instances — 4

Never a pattern, always a named instance quoting the exact sentence. Edit the
sentence and the exemption stops applying. The self-test fails if a grandfather
matches nothing, so a stale one cannot sit here unnoticed.

| Rule | Instance | Why |
|---|---|---|
| `claim-uncontrolled-verb` | `docs/06_RESEARCH_AGENDA.md` — "All synthesized maps are verified independently." | no scope, corpus, or claim row; pay off by naming the checker |
| `claim-verb-discipline` | `docs/52_RELEASE_GATES_REV3.md` — the G2 bullet "Context Packs are bounded, …" | boundedness asserted with no budget; pay off by naming the pack budget |
| `claim-verb-discipline` | `plan.md` — plan §22's mirror of the same G2 bullet | listed separately so paying one off does not silently cover the other |
| `claim-state-declared` | `docs/18` — `C025` in state `OBSERVED-BOUNDED` | a state docs/18 never declares; independently reported in `plan.review.1.md` |

## Fixtures

Nineteen, under `fixtures/claims/`. Each declares in an HTML comment the rule it
must trigger; a fixture that is not caught fails `--self-test`, so the check
cannot pass by detecting nothing. `claim-clean.md` is the negative control: all
seven verbs used correctly, and it must produce no finding at all.

## Wiring it into the gate

The lead owns the `Justfile`. The intended recipe, matching the `boundaries`
idiom (self-test first, so the check cannot pass vacuously):

```just
claims:
    python3 tools/governance/check_claim_governance.py --self-test
    python3 tools/governance/check_claim_governance.py --evidence
```
