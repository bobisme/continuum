# R14 — "Alien Math" Theater — Risk Checkpoint Evidence

**Prepared:** 2026-08-05 (bone `bn-33iv`)
**Scope:** mechanical evidence for the five controls
[`docs/08_RISK_REGISTER.md:218-224`](../docs/08_RISK_REGISTER.md) requires
against R14's failure mode —

> sheaves, homology, HDA, category theory appear in marketing without
> improving the system

— restated identically on the owning bone (`bn-33iv`): research labels;
baseline and threshold; no correctness claim from heuristic topology; kill
criteria; publish negative results.

This is a risk **checkpoint**, not a delivery: it verifies each control is
live today and states, per control, exactly which claim is mechanically
checked versus which rests on document discipline that no validator reads.
It decides nothing and adds no new source code — R14's failure mode is
about claims outrunning evidence, and the checkpoint's own job is therefore
to cite, not to build.

---

## 1. Control → evidence table

| # | Control | Status | Primary evidence |
|---|---|---|---|
| 1 | Research labels | **LIVE** | Closed `HYPOTHESIS`/`DESIGN`/... claim lattice (§1.1) applied to every alien-math claim (§1.1.1); zero shipped-capability language in product-facing docs (§1.1.2); `check_claim_governance.py` scans the public surface for uncontrolled claim verbs (§1.1.3) |
| 2 | Baseline and threshold | **LIVE** | §24.5 frontier lane register, 21 rows, 6 ratified `quote-id` marks; `check_register_rows()` enforces resolvability, a kill/defer/draft marker per row, and register↔note quote identity (§1.2) |
| 3 | No correctness claim from heuristic topology | **LIVE**, confinement currently vacuous — honest gap noted | INV-004/INV-008 delivered evidence: the four-kernel vocabulary sweep proves `is_verified` is the only boolean projection anywhere in the trusted checking base; `KCOV-06` proves the four kernel crates admit zero dependencies, so nothing heuristic or topological — including `heuristic-cutoff` — can reach a verdict (§1.3) |
| 4 | Kill criteria | **LIVE** | Per-idea kill conditions in `research/07` (all four core alien-math ideas); per-lane kill/defer clauses enforced by the same `check_register_rows()` marker rule; ADR-0020's binding decision sentence (§1.4) |
| 5 | Publish negative results | **PARTIAL — policy live, no fired instance yet** | ADR-0020 consequence clause; `REFUTED` claim state with mandated wording; docs/12 §9 publication strategy names negative sheaf/cubical results explicitly; no lane has reached a kill decision yet in Phase A, so there is no live `REFUTED` row to point at (§1.5) |

---

### 1.1 Control 1 — Research labels

**1.1.1 — Every alien-math claim carries `HYPOTHESIS`, never a stronger word.**

`docs/18_CLAIMS_MATRIX.md`'s seven-state claim lattice (`HYPOTHESIS`,
`TARGET`, `OBSERVED`, `ESTABLISHED-BOUNDED`, `ESTABLISHED`, `BLOCKED`,
`REFUTED`) is the mechanism; the alien-math rows are C014-C017:

```
C014  Observer-sensitive DPOR improves reduction soundly      HYPOTHESIS
C015  Cubical reduction outperforms optimal DPOR (high-width)  HYPOTHESIS
C016  Sheaf gluing yields useful compositional diagnostics     HYPOTHESIS
C017  Topological coverage improves bug yield                  HYPOTHESIS
```

(`docs/18_CLAIMS_MATRIX.md:32-35`). The documentation-rule table two
sections below states the exact allowed wording per state — `HYPOTHESIS`
admits only "we hypothesize," "research target" — and closes with: "CI
should reject bare 'verified,' 'sound,' 'complete,' 'deterministic,' or
'production-equivalent' unless linked to an adequate claim row"
(`docs/18_CLAIMS_MATRIX.md:52-54`).

`research/07-alien-mathematics.md` — the alien-math portfolio itself —
opens with the same discipline stated as a standing rule rather than a
per-claim tag: **"Status:** exploratory; none of these ideas are product
claims" / "**Rule:** mathematics survives only by producing a sound
algorithm, smaller evidence, better diagnostics, or measurable
verification power" (`research/07-alien-mathematics.md:3-4`), and closes
with the same nine-item governance list applied to every lane in the
note: formal statement, baseline algorithms, public benchmark corpus,
predeclared success metric, implementation budget, falsification/kill
criterion, `OBSERVED`/`HYPOTHESIS`/`PROVEN` classification, and **"no
effect on stable semantics until an ADR promotes it"**
(`research/07-alien-mathematics.md:221-233`).

**1.1.2 — No README or product doc claims a frontier capability as shipped.**

Checked directly: `sheaf|sheaves|homology|cohomolog|HDA|category.theor`
appear nowhere in the repository root `README.md`, `notes/plan/README.md`,
or any file under `crates/**` (recursive case-insensitive grep, zero
hits). No topology/homology/sheaf code exists anywhere in the Rust
workspace today — the confinement claim in control 3 is currently
vacuously true because there is nothing yet to confine, which is itself
the desired state at this program stage (see the honest gap recorded in
§1.3).

The repository root `README.md` states its phase explicitly rather than
implying delivery: "Continuum is currently in the **design and
executable-spike phase**. ... The production Rust crates, `continuumd`,
and the commands shown below are planned — not yet released."
(`README.md:12-15`).

Where the vocabulary *does* appear outside `research/`, it is inside
documents whose entire function is to keep claims honest, never inside a
capability description: `docs/18_CLAIMS_MATRIX.md` (C014-C017, all
`HYPOTHESIS`, quoted above), `docs/21_NOVEL_CONTRIBUTIONS.md` ("Cubical/
sheaf methods only after positive evidence" — item 6 of a numbered
sequencing list, and "A failed hypothesis is still valuable if the
benchmark and negative result are rigorous," `docs/21:79,81`),
`docs/12_GOVERNANCE_AND_ENGINEERING.md` §9 (publication strategy, see
§1.5), `docs/06_RESEARCH_AGENDA.md`, `docs/07_BENCHMARKS_AND_EVALUATION.md`
(the research scorecard documents ADR-0020 names as governing promotion),
`docs/14_GLOSSARY.md`, `docs/13_BIBLIOGRAPHY.md` (citation entries), and
`docs/20_ADVERSARIAL_DESIGN_REVIEW.md`/`docs/24_REVISION_2_ARCHITECTURE.md`/
`archive/plan-revision-2.md` (historical/adversarial review text, not
current product claims).

**1.1.3 — A mechanical scanner, not just a table, enforces claim discipline.**

`tools/governance/check_claim_governance.py` (docs/12 §3's "CI cross-checks
claim IDs" sentence made executable) scans the repository README, the
dossier README, all 55 `notes/plan/docs/*.md` files, and `plan.md` for:
the seven controlled verbs used with the discipline §3 requires (a scope,
corpus, bound, or assumption named); any of the closed set of
uncontrolled verbs ("verified," "guaranteed," "proven," "sound,"
"complete," "deterministic," "production-equivalent") used bare, outside
a hedge or a linked claim row; and claim-ID cross-reference integrity
between `docs/18` and its generated mirror in `PLAN_REQUIREMENTS.json`.
Reproduced live for this checkpoint:

```
$ python3 tools/governance/check_claim_governance.py --self-test   # 13 fixtures, all caught
{ "status": "pass", ... }
$ python3 tools/governance/check_claim_governance.py               # real repository
{
  "claim_rows": 35,
  "scanned_documents": 58,
  "violations": [],
  "status": "pass"
}
```

This is the general overclaiming gate (bare "verified"/"sound"/etc.), not
an alien-math-specific scanner — it would catch a sheaf/homology claim
phrased with a controlled or uncontrolled verb, but a claim phrased
outside its matched grammar (a nominalization, a table cell, a figure
caption — the tool's own stated limitation, `check_claim_governance.py`
docstring "Scope and honesty about limits") would not be seen. §1.1.2's
direct grep is the complementary check for exactly that blind spot on
the alien-math vocabulary specifically, and it is clean.

---

### 1.2 Control 2 — Baseline and threshold

`plan.md` §24.5 ("Frontier lane register") is the single authority: "Every
HYPOTHESIS-class capability in this plan is owned by a research lane with
a baseline, a quantitative promotion threshold, a kill criterion, and a
named fallback. The register is authoritative for lane status; no plan
section may claim a lane's output without its status." (`plan.md:2638-2641`).
Every register row carries a **Fallback** column, which for most rows *is*
the named baseline/current-state alternative the frontier claim must beat
(e.g. "plain causal slice + expansion," "conservative unreduced
exploration," "1-minimal delta debugging only," "SC-only" — `plan.md`
§24.5 table).

The sheaf-specific row states its baseline directly: **Sheaf-based
composition** | `research/03; docs/31` | "docs/31's stated promote/kill
pair" | fallback "monolithic composition proofs" — i.e. the sheaf lane is
compared against the composition approach it would replace, not evaluated
in isolation.

**Mechanical enforcement**, `notes/plan/tools/validate_dossier.py
check_register_rows()`:

- every row's threshold cell must contain one of `kill`/`defer`/`draft`/
  `fallback` (fail-closed — a row with a bare threshold and no
  kill/defer/draft marker fails the gate);
- every `research/NN` and `ADR-NNNN` reference in a row's `Lane` cell must
  resolve to a real file;
- **marked-quote identity**: a `quote-id=<slug> "<text>"` marker in the
  register must match, verbatim (whitespace-normalized), a marker of the
  same slug in some file under `research/*.md` — this is the register↔note
  identity check the task brief names directly. Zero markers anywhere
  means nothing is ratified; a mismatched quote is a hard failure, not a
  warning.

Reproduced live for this checkpoint (`notes/plan/` via
`uv run --with jsonschema python3 tools/validate_dossier.py`):

```
"register_rows": { "rows": 21, "ratified_quotes_checked": 6 },
"status": "pass"
```

21 rows matches `plan.md`'s current table exactly (context compilation,
exploration reduction, liveness-preserving reduction, causal minimization,
neighborhood adequacy, sub-file incremental trust, Forge co-synthesis+QD,
production conformance, cancellation calculus, bidirectional lenses,
proof repair, certificate overhead, nominal/orbit-finite verification,
sheaf-based composition, full TLA+ importer, weak-memory lane,
timed/probabilistic semantics, agent-computer interface, explanation
science, workbench security, multi-agent evidence-graph enforcement). The
6 ratified quotes are `context-compilation-win-margin`,
`causal-minimization-core-ratio`, `neighborhood-adequacy-catch-rate`,
`aci-benchmark-margins`, `workbench-security-promotion-gate`,
`evidence-graph-enforcement`.

`bn-14dqx` (closed 2026-08-04) synced `plan.typ`'s copy of the same table
to `plan.md`'s ratified wording — `plan.typ` is not read by the validator,
so this was cosmetic-build hygiene, not a gate requirement, but it means
the two documents no longer disagree on which six rows are ratified.

**Note on scope**: control 2's threshold/baseline discipline applies to
all 21 HYPOTHESIS-class lanes program-wide, of which the alien-math
portfolio (sheaves, homology, cubical/topological reduction, category
theory) is a named subset — the sheaf-based composition row above, plus
"exploration reduction" and "liveness-preserving reduction," whose
research notes (`research/01`, `research/13`) draw on the directed-topology
material `research/07` opens with. R14's control is the general register
discipline applied to this subset, not a separate alien-math-only
mechanism — which is consistent with `plan.md`'s own framing: the register
is the *one* lane authority, deliberately not duplicated per risk.

---

### 1.3 Control 3 — No correctness claim from heuristic topology

**The verdict authority is closed and proven closed.**
`INV-004` ("No self-certification," delivered `bn-11yx`, `bn-24i`,
`bn-3sypm`, `bn-bou09`) composes the four `continuum-kernel-*` crates
behind one entry point (`continuum-certificate`) that "takes `&[u8]` and
nothing else, routed by family magic taken from the kernels themselves,
each verdict relayed as the kernel's own value with `Unsupported` never
weakened into `Rejected`" (`plan.md:320`). `continuum-certificate`'s own
`Cargo.toml` states the dependency shape directly: "Nothing else is
declared, and nothing external is" beyond the four kernel crates
(`crates/continuum-certificate/Cargo.toml:9-16`).

`INV-008` ("Typed inconclusiveness," delivered `bn-n9a1`) adds the
cross-kernel sweep that closes the specific gap R14 is about — a heuristic
or topological signal masquerading as a verdict via a second boolean
escape hatch:

> `positive_is_verified_is_the_only_boolean_projection_in_every_kernel_crate`:
> sweeps for a second boolean (`is_rejected`/`is_unsupported`) that would
> let a caller re-collapse the distinction; pins `claim()` folding both to
> `None`

(`crates/continuum-kernel-core/tests/inv008_unsupported_vs_rejected_evidence.rs:224`,
cited in the bn-n9a1 delivery comment as "the four-kernel vocabulary
sweep"). Reproduced live: the test exists at that path and function name
today (confirmed by direct grep of the file), alongside
`positive_all_four_kernel_crates_declare_the_same_three_arm_verdict_vocabulary_in_order`
(line 175) — nothing outside `Verified`/`Rejected`/`Unsupported` can reach
a caller from any of the four kernel crates.

**The confinement is structural, not just tested.**
`tools/check_kernel_covenant.py` KCOV-06 ("dependency-freedom") enforces
that the four kernel crates' manifests declare dependencies only from
`ADMITTED_KERNEL_DEPENDENCIES`, which is the empty frozenset —
"the four kernel crates depend on nothing at all, so their decoder,
arithmetic and state representation are their own" (source comment,
`check_kernel_covenant.py:99-103`). `tools/check_crate_boundaries.py`'s
`certificate-checker-not-search` rule additionally forbids the checker
(kernel crates + `continuum-certificate`) from reaching any search engine
or Forge transitively. Between the two: nothing — heuristic, topological,
or otherwise — can be imported into the verdict path at all, mechanically,
not by convention.

Reproduced live for this checkpoint:

```
$ python3 tools/check_kernel_covenant.py --self-test   # 21 fixtures, all caught
{ "self_test": "pass" }
$ python3 tools/check_kernel_covenant.py
{ "passing": ["KCOV-01".."KCOV-09"], "failing": {}, "status": "pass" }
$ python3 tools/check_crate_boundaries.py --self-test   # 15 cases, all caught
{ "self_test": "pass" }
$ python3 tools/check_crate_boundaries.py
{ "violations": [], "status": "pass" }
```

**Where heuristic/topological code actually lives today, and why that is
outside the confinement boundary.** The only shipped "heuristic" surface
in the codebase is `OmissionReason::HeuristicCutoff`
(`crates/continuum-context/src/omission.rs:88-113`) — RFC 0028's
context-compiler omission reason for "dropped by the stage-9 ranker; the
compiler could not decide." This is a Context Pack bookkeeping type, not a
verification verdict: it lives in `continuum-context`, a crate the kernel
covenant's zero-dependency rule (KCOV-06) makes structurally unreachable
from any of the four kernel crates, and which `continuum-certificate`
does not depend on either (its `Cargo.toml` names only the four kernel
crates, quoted above). The context-compilation frontier lane itself
(§24.5 row "General context compilation") is ratified with a
win-margin/generalization threshold measured on task-success and
context-byte metrics — an agent-benchmark outcome, never a soundness
claim — so `heuristic-cutoff` never enters the confinement question as
anything but an omission-accounting label.

**Honest gap.** No sheaf, homology, cohomology, persistent-homology,
directed-topology, or category-theoretic code exists anywhere in
`crates/**` today (confirmed by recursive case-insensitive grep, zero
hits — same search as §1.1.2). The confinement claim above is therefore
currently **vacuously true**: there is no topological code in the
workspace to confine, heuristic or otherwise, beyond `heuristic-cutoff`'s
omission-accounting use. That is the correct state for Phase A — `research/07`'s
governance list is explicit that a lane has "no effect on stable
semantics until an ADR promotes it" — but it means control 3's mechanical
guarantee has not yet been exercised against a real topological
implementation attempt. The guarantee that matters is the one already
proven: the kernel covenant's zero-dependency rule and the certificate
crate's four-crate-only manifest would refuse such an import structurally,
the day one is attempted, without needing a new rule written for that day.

---

### 1.4 Control 4 — Kill criteria

Every one of `research/07`'s core alien-math ideas states an explicit kill
condition inline:

- **Directed topology / concurrency geometry**: "No consistent win over
  optimal DPOR/unfoldings on systems with high causal width."
  (`research/07-alien-mathematics.md:23`)
- **Sheaves and cohomological obstruction**: "A direct SAT/CSP formulation
  is simpler, faster, and equally diagnostic." (`:44`)
- **Homological schedule coverage**: "No held-out improvement over
  pair/event/state coverage," plus the standing caveat "Coverage is not
  correctness. Homology can be an attractive dashboard with no
  bug-finding value." (`:56,53`)
- **Category-theoretic semantics**: governed by the "Discipline" section
  restricting it to specification/proofs unless it materially simplifies
  code (`:80`).

`docs/31_FALSIFICATION_AND_KILL_CRITERIA.md` restates two of these with
numeric promote/kill pairs ("Higher-dimensional/cubical reduction": "≥3×
memory/time improvement on at least two real protocol classes with a
preservation theorem" / "Kill if: only synthetic grid benchmarks win..."; "Sheaf
gluing": promote if it "proves or diagnoses a multi-component case that
pairwise checks cannot" / "Kill if: it restates constraint solving less
efficiently or explanations require specialists," `docs/31:14-25`).

`ADR-0020` ("Gate speculative mathematics behind falsifiable research
programs") is the binding decision record: "Experimental engines are
separately named, benchmarked, and assurance-capped. Each has a
hypothesis, baseline, threshold, and kill criterion. No public soundness
claim relies on them before independent validation." Its alternatives
section explicitly rejects the R14 failure mode by name: "Use
mathematical vocabulary only for marketing: explicitly rejected."
(`adr/0020-experimental-mathematics-governance.md:13,23`).

**Mechanical enforcement**: the same `check_register_rows()` fail-closed
marker rule cited in §1.2 requires every §24.5 register row — including
the sheaf-based-composition row — to carry a `kill`/`defer`/`draft`/
`fallback` token in its threshold cell; a row that dropped its kill
clause would fail the dossier gate, not merely a review. `check_adr_process.py`
(part of `just check`'s `governance` target, confirmed passing in this
checkpoint's run) mechanically requires every ADR — including ADR-0020 —
to carry nine sections in order, among them "validation plan" and
"rollback"; ADR-0020's combined "## Validation and rollback" heading
satisfies that structural requirement.

---

### 1.5 Control 5 — Publish negative results (partial: policy live, unexercised)

**What is live.** `ADR-0020`'s consequences clause states the policy
directly: "Continuum can pursue frontier research without contaminating
the product or credibility. **Negative results are acceptable outputs.**"
(`adr/0020-experimental-mathematics-governance.md:17`). `docs/18`'s claim
lattice has a dedicated `REFUTED` state whose mandated wording is
"explicitly document the negative result" (`docs/18_CLAIMS_MATRIX.md:52`)
— refutation is a first-class, checkable claim state, not an omission.
`docs/12_GOVERNANCE_AND_ENGINEERING.md` §9 ("Publication strategy") names
alien-math negative results specifically among the program's intended
research outputs: "sheaf/cubical experimental results, **including
negative findings**" (`docs/12:181`). `docs/21_NOVEL_CONTRIBUTIONS.md`
states the same discipline as a value judgment, not just a policy: "A
failed hypothesis is still valuable if the benchmark and negative result
are rigorous." (`docs/21:81`).

**What is not yet exercised.** No frontier lane — alien-math or otherwise
— has reached a kill decision in the program's current state (Phase A;
`plan.md` §0.3's program status is `READY`, not "phases complete"). There
is therefore no live example of a `REFUTED` claim row, no fired
`research/07` kill condition, and no case where `check_claim_governance.py`'s
`claim-state-declared` rule has had to validate a real `REFUTED` state
transition end to end. This is not evidence of avoidance — nothing in the
alien-math portfolio has run a benchmark yet, so nothing has had the
chance to be refuted — but it means control 5, unlike controls 1-4, rests
entirely on document discipline with no mechanical validator that checks
"a fired kill criterion produced a published negative result." A future
risk-checkpoint pass, once any §24.5 lane reaches its kill decision,
should verify the `REFUTED` row and its negative-result document actually
land, not merely that the policy sentence still reads correctly.

---

## 2. What this document does NOT do

- It does not modify `docs/08_RISK_REGISTER.md`'s R14 row. That file's
  existing rows carry no "(delivered: ...)" or evidence-citation
  convention anywhere (checked against all 21 rows, R01-R21) — unlike `plan.md`'s
  `INV-NNN` list, which does use that convention. Per this checkpoint's
  own instruction to touch the row only per its established conventions,
  the row is left untouched; evidence is recorded here and on the bone
  instead. This is a stated divergence from the assumption that the
  checkpoint would annotate the register row.
- It does not add or modify any Rust source. R14's failure mode is claims
  outrunning evidence; the checkpoint's job is citation and verification
  of controls already delivered by other bones (`bn-14dqx`, `bn-11yx`,
  `bn-24i`, `bn-3sypm`, `bn-bou09`, `bn-n9a1`), not new product surface.
- It does not decide whether R14 is "closed." A risk checkpoint verifies
  controls are live; retiring a program risk is the privileged decision
  `plan.md` §24.5 and the bone's own acceptance criteria reserve to the
  human lead, via the parent goal `bn-34a8` ("Risk retirement, kill-signal
  assays, and scope decisions").
- It does not speak to any other risk row (R01, R16, ...). Sibling risk
  bones (`bn-1os6`/R01, `bn-3j4g`/R16) are concurrent, independent
  checkpoints against different controls; this document is scoped to R14
  only and was written without reading or depending on their evidence
  artifacts.
