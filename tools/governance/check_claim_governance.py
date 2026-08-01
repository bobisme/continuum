#!/usr/bin/env python3
"""Mechanically enforce Continuum's claim governance (docs/12 §3).

Source of truth:

- `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §3 "Claim governance" — the
  seven controlled verbs and the sentence "CI cross-checks claim IDs.";
- `notes/plan/notes/PLAN_REQUIREMENTS.json` — the per-ID entries `GOV-3-01` …
  `GOV-3-07` (one per verb) and the 35 `claim`-category entries `C001` … `C035`;
- `notes/plan/docs/18_CLAIMS_MATRIX.md` — the claim registry itself: the claim
  rows, the closed list of evidence states, the state→wording table, and the
  rule "CI should reject bare “verified,” “sound,” “complete,” “deterministic,”
  or “production-equivalent” unless linked to an adequate claim row."

The claim registry is not invented here. `docs/18` is the registry; the `claim`
category of `PLAN_REQUIREMENTS.json` is its generated mirror; `.bones/events`
is where each active claim is owned by a work item. All three are cross-checked
against each other rather than replaced.

Rules, and the obligation each serves:

1. RULE claim-verb-registered (GOV-3-01 … GOV-3-07, one per verb)
   docs/12 §3 lists exactly the seven controlled verbs, in order, and the
   `GOV-3-0N` requirement entry for each names that verb. Adding an eighth verb,
   renaming one, or letting the generated registry drift fails here.
2. RULE claim-verb-discipline (GOV-3-01 … GOV-3-06, one per verb)
   Where a scanned public document asserts a claim *with* a controlled verb, the
   verb carries the discipline §3 implies: `observed` names its scope, `tested`
   names its corpus, `bounded` and `exhaustively checked` carry their bound,
   `proved under` names its assumptions, `certificate checked` names the checked
   artifact, and `hypothesized` does not co-occur with a stronger claim word.
3. RULE claim-uncontrolled-verb (the closed-set half of GOV-3-01 … GOV-3-06)
   Where a scanned public document asserts a verification outcome with a verb
   from *outside* the seven — bare "verified", "guaranteed", "proven", "sound",
   "complete", "deterministic", "production-equivalent" — it fails unless the
   assertion is linked to a claim row, states its scope, or is hedged.
4. RULE claim-id-resolves / claim-registry-mirror / claim-registered-referenced
   / claim-state-declared (the "CI cross-checks claim IDs" sentence of §3,
   recorded against all seven IDs)
   Claim IDs cited in scanned documents resolve to registry rows; the registry
   and its generated mirror agree in both directions, row for row; every active
   registered claim is referenced at least once outside the registry; every
   registry row's evidence state is one docs/18 declares.

Scope and honesty about limits:

- Scanned documents are the repository README, the dossier README, the 55
  `notes/plan/docs/*.md` documents, and `notes/plan/plan.md` — the public,
  claim-bearing surface. RFCs, ADRs, research notes, review transcripts, the
  archive, and source comments are NOT scanned. A claim smuggled into an RFC is
  outside this check.
- Fenced code blocks, inline code spans, and curly-quoted strings are removed
  before scanning: they carry artifact text, CLI renderings, and vocabulary
  citations, not assertions. docs/12 §3's own verb list and docs/18's
  state→wording table are therefore *defined* here, not *scanned* here.
- Rules 2 and 3 are proxies. They match a claim-assertion grammar over English
  prose (copula or perfect + the verb in predicate position) with a fixed
  exemption set. A claim phrased in a construction the grammar does not model —
  a nominalization, a table cell without a copula, a figure caption — is not
  seen. Each evidence entry records this in its `boundary` field.
- Grandfathered instances are named individually below, never by pattern, and
  their count is reported in the run output and the evidence file.

Stdlib only. Exit 0 when every rule holds, 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
from dataclasses import dataclass, field

TOOL_DIR = pathlib.Path(__file__).resolve().parent
ROOT = TOOL_DIR.parents[1]
FIXTURES = TOOL_DIR / "fixtures" / "claims"
EVIDENCE = TOOL_DIR / "evidence" / "gov-3.json"

DOCS_12 = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
DOCS_18 = "notes/plan/docs/18_CLAIMS_MATRIX.md"
REQUIREMENTS = "notes/plan/notes/PLAN_REQUIREMENTS.json"
BONES_EVENTS = ".bones/events"

# --- docs/12 §3, expanded exactly ---------------------------------------------
#
# The closed seven-member set, in the order §3 lists it. The index is the
# obligation ID: verb i is GOV-3-0(i+1) in PLAN_REQUIREMENTS.json.

CONTROLLED_VERBS: tuple[str, ...] = (
    "observed",
    "tested",
    "bounded",
    "exhaustively checked",
    "proved under",
    "certificate checked",
    "hypothesized",
)

OBLIGATION_IDS: tuple[str, ...] = tuple(f"GOV-3-{i:02d}" for i in range(1, 8))
VERB_BY_ID: dict[str, str] = dict(zip(OBLIGATION_IDS, CONTROLLED_VERBS))

# The §3 sentence that bn-18cg owns. It carries no requirement ID of its own in
# PLAN_REQUIREMENTS.json, so its rules are recorded against all seven IDs.
CROSS_CHECK_SENTENCE = "CI cross-checks claim IDs."

# --- docs/18, expanded exactly -------------------------------------------------

# The five words docs/18 names, plus the two the bone names ("guaranteed",
# "proven"). Every entry is a verb that asserts a verification outcome from
# outside the controlled seven.
UNCONTROLLED_TERMS: dict[str, str] = {
    "verified": "docs/18 documentation rule",
    "sound": "docs/18 documentation rule",
    "complete": "docs/18 documentation rule",
    "deterministic": "docs/18 documentation rule",
    "production-equivalent": "docs/18 documentation rule",
    "guaranteed": "bn-227c",
    "proven": "bn-227c",
    "proved": "bn-227c",
}

# The evidence states docs/18 declares. Parsed from the document at run time;
# this copy is the self-test's fixed point and the drift detector.
DECLARED_STATES: tuple[str, ...] = (
    "HYPOTHESIS",
    "TARGET",
    "OBSERVED",
    "ESTABLISHED-BOUNDED",
    "ESTABLISHED",
    "BLOCKED",
    "REFUTED",
)

# --- scan scope ----------------------------------------------------------------

SCAN_GLOBS: tuple[str, ...] = (
    "README.md",
    "notes/plan/README.md",
    "notes/plan/docs/*.md",
    "notes/plan/plan.md",
)

# --- grandfathered instances ---------------------------------------------------
#
# Each entry names one instance, quoting the exact sentence. The quote is the
# key: edit the sentence and the grandfather stops applying, so these can only
# be paid off, never silently reused. Counts are reported prominently.

@dataclass(frozen=True)
class Grandfather:
    rule: str
    path: str
    text: str
    reason: str


GRANDFATHERED: tuple[Grandfather, ...] = (
    Grandfather(
        rule="claim-uncontrolled-verb",
        path="notes/plan/docs/06_RESEARCH_AGENDA.md",
        text="All synthesized maps are verified independently.",
        reason=(
            "Research-agenda C3 states the independence requirement for synthesized "
            "abstraction maps with no scope, corpus, or claim row. Pay off by naming "
            "the checker and the claim row, or by restating with a controlled verb."
        ),
    ),
    Grandfather(
        rule="claim-verb-discipline",
        path="notes/plan/docs/52_RELEASE_GATES_REV3.md",
        text=(
            "- Context Packs are bounded, carry omission manifests and expansion handles, "
            "and improve agent benchmark effectiveness;"
        ),
        reason=(
            "G2 exit criterion. The bullet asserts boundedness of the artifact class with no "
            "budget, so a reader cannot size it. Pay off by naming the pack budget the gate "
            "measures against."
        ),
    ),
    Grandfather(
        rule="claim-verb-discipline",
        path="notes/plan/plan.md",
        text=(
            "- Context Packs are bounded, carry omission manifests and expansion handles, "
            "and improve agent benchmark effectiveness;"
        ),
        reason=(
            "plan §22's mirror of the same G2 criterion. Both copies are grandfathered "
            "separately so paying one off does not silently cover the other."
        ),
    ),
    Grandfather(
        rule="claim-state-declared",
        path=DOCS_18,
        text="C025 OBSERVED-BOUNDED",
        reason=(
            "docs/18 C025 carries the state OBSERVED-BOUNDED, which its own evidence-state "
            "list does not declare. Known defect, independently reported in "
            "plan.review.1.md ('docs/18 C025 uses an undeclared state'). Pay off by "
            "declaring the state or by restating C025 as ESTABLISHED-BOUNDED."
        ),
    ),
)


# --- markdown reduction --------------------------------------------------------

FENCE = re.compile(r"^\s*(?:```|~~~)")
BLOCK_START = re.compile(r"^\s*(?:[-*+]\s|\d+[.)]\s|\||>|#|---\s*$)")
INLINE_CODE = re.compile(r"`[^`]*`")
CURLY_QUOTED = re.compile(r"[“][^”]*[”]")
HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)


@dataclass
class Block:
    """One markdown block (paragraph, list item, table row) joined to one line."""

    text: str
    line: int


def blocks(text: str) -> list[Block]:
    """Reduce a markdown document to scannable prose blocks.

    Fenced code is dropped whole. Hard-wrapped continuation lines are rejoined so
    a sentence split across source lines is seen as one sentence. Inline code
    spans, curly-quoted strings, and HTML comments become a placeholder: they
    cite vocabulary, they do not assert.
    """
    text = HTML_COMMENT.sub(lambda m: "\n" * m.group(0).count("\n"), text)
    out: list[Block] = []
    current: list[str] = []
    start = 0
    in_fence = False

    def flush() -> None:
        nonlocal current, start
        if current:
            joined = " ".join(s.strip() for s in current).strip()
            if joined:
                out.append(Block(text=joined, line=start))
        current = []

    for lineno, raw in enumerate(text.splitlines(), 1):
        if FENCE.match(raw):
            in_fence = not in_fence
            flush()
            continue
        if in_fence:
            continue
        stripped = raw.strip()
        if not stripped:
            flush()
            continue
        if BLOCK_START.match(raw) or not current:
            flush()
            start = lineno
        current.append(stripped)
    flush()

    reduced: list[Block] = []
    for block in out:
        body = INLINE_CODE.sub(" ⟨code⟩ ", block.text)
        body = CURLY_QUOTED.sub(" ⟨quote⟩ ", body)
        body = body.lstrip("#> ").strip()
        if body:
            reduced.append(Block(text=body, line=block.line))
    return reduced


SENTENCE_SPLIT = re.compile(r"(?<=[.;!?])\s+")


def sentences(block: Block) -> list[str]:
    return [s for s in SENTENCE_SPLIT.split(block.text) if s.strip()]


# --- the claim-assertion grammar ----------------------------------------------

COPULA = r"(?:is|are|was|were|has been|have been|had been|remains|remain|becomes|become)"
LEADING_ADVERB = r"(?:\s+(?:already|now|independently|fully|formally|therefore|thus|also|itself|hereby|provably|demonstrably|mechanically|statically|exhaustively))*"
TRAILING_ADVERB = re.compile(r"^\s*(?:\w+ly)\b")

# A qualifier names scope, evidence, assumptions, or a bound. Its presence is what
# separates a disciplined claim from a bare one.
QUALIFIER_HEAD = re.compile(
    r"^\s*[,;:]?\s*(?:under|over|within|for|on|by|through|via|against|across|"
    r"up to|in|with|when|where|from|per|except|according|below|above|at)\b(?:\s|:|$)",
    re.IGNORECASE,
)
CLAUSE_END = re.compile(r"^\s*(?:[.,;:!?)\]]|and\b|or\b|but\b|yet\b|so\b|—|–|$)")

CLAIM_ID = re.compile(r"\bC\d{3}\b")
NEGATION = re.compile(
    r"\b(?:not|never|no|cannot|can't|rather than|instead|nor|without|except|neither)\b",
    re.IGNORECASE,
)
GOVERNING = re.compile(
    r"\b(?:if|unless|when|where|while|because|until|before|after|whether|assuming|"
    r"provided|must|should|may|might|would|will|shall|only|aims|intends|plans|"
    r"target|targets|goal|hypothesis|hypothesize|hypothesise)\b",
    re.IGNORECASE,
)
RELATIVE_SUBJECT = re.compile(
    r"^(?:what|whatever|which|whichever|who|whom|whose|that|nothing|anything|"
    r"something|everything|there)$",
    re.IGNORECASE,
)

NUMERIC = re.compile(
    r"\b\d|\b(?:zero|one|two|three|four|five|six|seven|eight|nine|ten)\b|[≤≥<>]",
    re.IGNORECASE,
)

# Multiword controlled verbs are unambiguous claim vocabulary and are recognised
# with or without a copula. Single-word ones need the assertive construction,
# because "bounded Context Packs" and "observed behavior" are ordinary prose.
MULTIWORD_VERBS = ("exhaustively checked", "proved under", "certificate checked")

# Words that occupy the assumption slot of `proved under` without naming anything.
PLACEHOLDER_NOUNS = frozenset(
    {
        "assumptions",
        "assumption",
        "conditions",
        "condition",
        "hypotheses",
        "hypothesis",
        "some",
        "certain",
        "various",
        "appropriate",
        "suitable",
        "them",
        "it",
        "this",
        "that",
    }
)

ARTIFACT = re.compile(
    r"\b(?:LRAT|DRAT|Alethe|receipt|witness|manifest|artifact|closure certificate|"
    r"refinement certificate|kernel|checker|\S+\.(?:json|cert|lrat|alethe|olean))\b",
    re.IGNORECASE,
)


@dataclass
class Finding:
    rule: str
    obligation: str
    path: str
    line: int
    text: str
    detail: str

    def render(self) -> str:
        return f"[{self.rule}] {self.path}:{self.line} — {self.detail}: “{self.text}”"


def _predicate_position(tail: str) -> tuple[bool, bool]:
    """Return (is_predicate, is_qualified) for the text following a matched verb."""
    rest = tail
    for _ in range(2):
        adverb = TRAILING_ADVERB.match(rest)
        if not adverb:
            break
        rest = rest[adverb.end():]
    if QUALIFIER_HEAD.match(rest):
        return True, True
    if CLAUSE_END.match(rest):
        return True, False
    return False, False


def _exemptions(sentence: str, block_text: str, match_start: int, qualified: bool) -> list[str]:
    reasons: list[str] = []
    if qualified:
        reasons.append("qualified")
    if CLAIM_ID.search(block_text):
        reasons.append("claim-link")
    if NEGATION.search(sentence):
        reasons.append("negated")
    if GOVERNING.search(sentence[:match_start]):
        reasons.append("hedged")
    return reasons


def _verb_qualified(sentence: str, tail: str, verb: str) -> tuple[bool, str]:
    """Does a controlled verb carry the discipline docs/12 §3 implies for it?"""
    if verb == "proved under":
        content = re.sub(r"^[\s,;:]+", "", tail)
        content = re.sub(r"^(?:the|a|an|its|our|their|these|those)\s+", "", content, flags=re.IGNORECASE)
        head = re.match(r"[\w'’\-]+", content)
        if not head or head.group(0).lower() in PLACEHOLDER_NOUNS:
            return False, "`proved under` must name the assumptions it is proved under"
        return True, ""
    if verb == "certificate checked":
        if ARTIFACT.search(sentence):
            return True, ""
        return False, "`certificate checked` must name the checked artifact or its checker"
    if verb in ("bounded", "exhaustively checked"):
        if NUMERIC.search(tail) or QUALIFIER_HEAD.match(tail):
            return True, ""
        return False, f"`{verb}` must carry the bound it holds within"
    if verb == "observed":
        if QUALIFIER_HEAD.match(tail):
            return True, ""
        return False, "`observed` must name the scope it was observed on"
    if verb == "tested":
        if QUALIFIER_HEAD.match(tail):
            return True, ""
        return False, "`tested` must name the corpus or suite it was tested against"
    if verb == "hypothesized":
        stronger = re.search(
            r"\b(?:verified|proved|proven|guaranteed|established|certificate checked|"
            r"exhaustively checked)\b",
            sentence,
            re.IGNORECASE,
        )
        if stronger:
            return False, "`hypothesized` may not share a claim with a stronger claim word"
        return True, ""
    return True, ""


def scan_document(path: str, text: str) -> tuple[list[Finding], dict[str, int]]:
    """Apply the claim-assertion grammar to one document."""
    findings: list[Finding] = []
    counts: dict[str, int] = {verb: 0 for verb in CONTROLLED_VERBS}
    counts["uncontrolled-candidates"] = 0

    verb_alt = "|".join(re.escape(v) for v in CONTROLLED_VERBS)
    term_alt = "|".join(re.escape(t) for t in UNCONTROLLED_TERMS)
    assertive_verb = re.compile(
        rf"\b(?P<subject>[\w'’\-]+)\s+(?:{COPULA})\b{LEADING_ADVERB}\s+(?P<verb>{verb_alt})\b",
        re.IGNORECASE,
    )
    multiword = re.compile(rf"\b(?P<verb>{'|'.join(re.escape(v) for v in MULTIWORD_VERBS)})\b", re.IGNORECASE)
    assertive_term = re.compile(
        rf"\b(?P<subject>[\w'’\-]+)\s+(?:{COPULA})\b{LEADING_ADVERB}\s+(?P<term>{term_alt})\b",
        re.IGNORECASE,
    )

    for block in blocks(text):
        for sentence in sentences(block):
            seen_spans: list[tuple[int, int]] = []

            # --- controlled verbs (GOV-3-01 … GOV-3-07 discipline)
            for match in list(assertive_verb.finditer(sentence)) + list(multiword.finditer(sentence)):
                verb = match.group("verb").lower()
                if verb not in CONTROLLED_VERBS:
                    continue
                if any(s <= match.start("verb") < e for s, e in seen_spans):
                    continue
                tail = sentence[match.end():]
                if verb not in MULTIWORD_VERBS:
                    is_predicate, _ = _predicate_position(tail)
                    if not is_predicate:
                        continue
                seen_spans.append((match.start("verb"), match.end()))
                # A conditional, negated, or normative sentence states a criterion,
                # not a claim; the per-verb discipline binds claims.
                if _exemptions(sentence, block.text, match.start(), False):
                    continue
                counts[verb] += 1
                ok, detail = _verb_qualified(sentence, tail, verb)
                if not ok:
                    obligation = OBLIGATION_IDS[CONTROLLED_VERBS.index(verb)]
                    findings.append(
                        Finding(
                            rule="claim-verb-discipline",
                            obligation=obligation,
                            path=path,
                            line=block.line,
                            text=sentence.strip()[:200],
                            detail=detail,
                        )
                    )

            # --- uncontrolled verbs (the closed-set half)
            for match in assertive_term.finditer(sentence):
                if any(s <= match.start("term") < e for s, e in seen_spans):
                    continue
                term = match.group("term").lower()
                tail = sentence[match.end():]
                if term in ("proved", "proven") and re.match(r"^\s+under\b", tail):
                    continue  # `proved under` — a controlled verb, handled above
                is_predicate, qualified = _predicate_position(tail)
                if not is_predicate:
                    continue
                if RELATIVE_SUBJECT.match(match.group("subject")):
                    continue
                counts["uncontrolled-candidates"] += 1
                exempt = _exemptions(sentence, block.text, match.start(), qualified)
                if exempt:
                    for reason in exempt:
                        key = f"exempted:{reason}"
                        counts[key] = counts.get(key, 0) + 1
                    continue
                findings.append(
                    Finding(
                        rule="claim-uncontrolled-verb",
                        obligation="GOV-3-01..06",
                        path=path,
                        line=block.line,
                        text=sentence.strip()[:200],
                        detail=(
                            f"bare “{term}” asserts a verification outcome outside the "
                            "controlled seven, with no scope, bound, or claim row"
                        ),
                    )
                )
    return findings, counts


# --- the registry --------------------------------------------------------------

CLAIM_ROW = re.compile(r"^\|\s*(C\d{3})\s*\|(?P<claim>.*?)\|(?P<evidence>.*?)\|(?P<state>.*?)\|\s*$")
STATE_BLOCK = re.compile(r"```text\n(?P<body>[A-Z\-\n]+?)```", re.MULTILINE)


@dataclass
class ClaimRow:
    id: str
    line: int
    claim: str
    evidence: str
    state: str


@dataclass
class Registry:
    rows: dict[str, ClaimRow] = field(default_factory=dict)
    states: tuple[str, ...] = ()


def normalize(text: str) -> str:
    return re.sub(r"[`“”\"]", "", text).strip()


def parse_registry(text: str) -> Registry:
    rows: dict[str, ClaimRow] = {}
    for lineno, line in enumerate(text.splitlines(), 1):
        match = CLAIM_ROW.match(line)
        if not match:
            continue
        rows[match.group(1)] = ClaimRow(
            id=match.group(1),
            line=lineno,
            claim=match.group("claim").strip(),
            evidence=match.group("evidence").strip(),
            state=match.group("state").strip(),
        )
    states: tuple[str, ...] = ()
    block = STATE_BLOCK.search(text)
    if block:
        states = tuple(s for s in (line.strip() for line in block.group("body").splitlines()) if s)
    return Registry(rows=rows, states=states)


def parse_verb_list(text: str) -> list[str]:
    """The bullet list under docs/12's `## 3. Claim governance` heading."""
    verbs: list[str] = []
    in_section = False
    for line in text.splitlines():
        if line.startswith("## "):
            if in_section:
                break
            in_section = line.strip().lower().endswith("claim governance")
            continue
        if not in_section:
            continue
        bullet = re.match(
            r"^-\s+`([^`]+)`\s*[;.]?\s*(?:\(delivered:[^)]*\)\s*)?$", line.strip()
        )
        if bullet:
            verbs.append(bullet.group(1).strip())
    return verbs


def section_text(text: str, heading_suffix: str) -> str:
    out: list[str] = []
    in_section = False
    for line in text.splitlines():
        if line.startswith("## "):
            if in_section:
                break
            in_section = line.strip().lower().endswith(heading_suffix)
            continue
        if in_section:
            out.append(line)
    return "\n".join(out)


def registry_findings(
    registry: Registry,
    mirror: dict[str, dict],
    references: dict[str, list[str]],
    registry_path: str = DOCS_18,
) -> list[Finding]:
    """Cross-check the registry against its mirror, its citations, and its states."""
    findings: list[Finding] = []

    # RULE claim-state-declared
    declared = set(registry.states) or set(DECLARED_STATES)
    for row in registry.rows.values():
        if row.state in declared:
            continue
        findings.append(
            Finding(
                rule="claim-state-declared",
                obligation="GOV-3-01..07",
                path=registry_path,
                line=row.line,
                text=f"{row.id} {row.state}",
                detail=(
                    f"evidence state “{row.state}” is not one the registry declares "
                    f"(claim: {row.claim})"
                ),
            )
        )

    # RULE claim-registry-mirror (bidirectional, row for row)
    for claim_id, row in sorted(registry.rows.items()):
        entry = mirror.get(claim_id)
        if entry is None:
            findings.append(
                Finding(
                    rule="claim-registry-mirror",
                    obligation="GOV-3-01..07",
                    path=registry_path,
                    line=row.line,
                    text=claim_id,
                    detail="registry row has no entry in the generated claim mirror",
                )
            )
            continue
        if normalize(entry.get("summary", "")) != normalize(row.claim):
            findings.append(
                Finding(
                    rule="claim-registry-mirror",
                    obligation="GOV-3-01..07",
                    path=registry_path,
                    line=row.line,
                    text=claim_id,
                    detail=(
                        f"registry text “{normalize(row.claim)}” and mirror summary "
                        f"“{normalize(entry.get('summary', ''))}” disagree"
                    ),
                )
            )
        source = entry.get("source", {})
        if source.get("line") != row.line:
            findings.append(
                Finding(
                    rule="claim-registry-mirror",
                    obligation="GOV-3-01..07",
                    path=registry_path,
                    line=row.line,
                    text=claim_id,
                    detail=f"mirror points at line {source.get('line')}, registry row is line {row.line}",
                )
            )
    for claim_id in sorted(set(mirror) - set(registry.rows)):
        findings.append(
            Finding(
                rule="claim-registry-mirror",
                obligation="GOV-3-01..07",
                path=REQUIREMENTS,
                line=0,
                text=claim_id,
                detail="claim exists in the generated mirror but has no registry row",
            )
        )

    # RULE claim-registered-referenced (every active claim is owned somewhere)
    for claim_id, row in sorted(registry.rows.items()):
        entry = mirror.get(claim_id, {})
        if entry.get("status", "active") != "active":
            continue
        if references.get(claim_id):
            continue
        findings.append(
            Finding(
                rule="claim-registered-referenced",
                obligation="GOV-3-01..07",
                path=registry_path,
                line=row.line,
                text=f"{claim_id} — {row.claim}",
                detail="active registered claim is referenced by no scanned document and no Bone",
            )
        )
    return findings


def id_resolution_findings(
    cited: dict[str, list[tuple[str, int, str]]], registry: Registry
) -> list[Finding]:
    """RULE claim-id-resolves — a cited claim ID that names no registry row."""
    findings: list[Finding] = []
    for claim_id, sites in sorted(cited.items()):
        if claim_id in registry.rows:
            continue
        for path, line, text in sites:
            findings.append(
                Finding(
                    rule="claim-id-resolves",
                    obligation="GOV-3-01..07",
                    path=path,
                    line=line,
                    text=text[:200],
                    detail=f"claim ID {claim_id} resolves to no row in the claim registry",
                )
            )
    return findings


def vocabulary_findings(verbs: list[str], mirror_verbs: dict[str, str]) -> list[Finding]:
    """RULE claim-verb-registered — the closed set, per obligation ID."""
    findings: list[Finding] = []
    if len(verbs) != len(CONTROLLED_VERBS):
        findings.append(
            Finding(
                rule="claim-verb-registered",
                obligation="GOV-3-01..07",
                path=DOCS_12,
                line=0,
                text=", ".join(verbs),
                detail=(
                    f"§3 lists {len(verbs)} controlled verbs; the closed set has "
                    f"{len(CONTROLLED_VERBS)}"
                ),
            )
        )
    for index, expected in enumerate(CONTROLLED_VERBS):
        obligation = OBLIGATION_IDS[index]
        actual = verbs[index] if index < len(verbs) else None
        if actual != expected:
            findings.append(
                Finding(
                    rule="claim-verb-registered",
                    obligation=obligation,
                    path=DOCS_12,
                    line=0,
                    text=str(actual),
                    detail=f"§3 verb {index + 1} is “{actual}”; {obligation} names “{expected}”",
                )
            )
        summary = normalize(mirror_verbs.get(obligation, "")).rstrip(".")
        if summary != expected:
            findings.append(
                Finding(
                    rule="claim-verb-registered",
                    obligation=obligation,
                    path=REQUIREMENTS,
                    line=0,
                    text=summary,
                    detail=f"{obligation} summary is “{summary}”; the controlled verb is “{expected}”",
                )
            )
    for extra in verbs[len(CONTROLLED_VERBS):]:
        findings.append(
            Finding(
                rule="claim-verb-registered",
                obligation="GOV-3-01..07",
                path=DOCS_12,
                line=0,
                text=extra,
                detail=f"§3 lists “{extra}”, which is not one of the controlled seven",
            )
        )
    return findings


# --- repository inputs ---------------------------------------------------------


def scan_paths(root: pathlib.Path) -> list[pathlib.Path]:
    paths: list[pathlib.Path] = []
    for glob in SCAN_GLOBS:
        if "*" in glob:
            paths.extend(sorted(root.glob(glob)))
        else:
            candidate = root / glob
            if candidate.exists():
                paths.append(candidate)
    return paths


def bone_references(root: pathlib.Path) -> dict[str, list[str]]:
    """Claim IDs named by Bones in the event log (the ownership half of the ledger)."""
    refs: dict[str, list[str]] = {}
    events = root / BONES_EVENTS
    if not events.is_dir():
        return refs
    for path in sorted(events.glob("*.events")):
        text = path.read_text(encoding="utf-8", errors="replace")
        for claim_id in set(CLAIM_ID.findall(text)):
            refs.setdefault(claim_id, []).append(f"bones:{path.name}")
    return refs


def grandfather_filter(
    findings: list[Finding], grandfathers: tuple[Grandfather, ...]
) -> tuple[list[Finding], list[dict]]:
    remaining: list[Finding] = []
    applied: list[dict] = []
    for finding in findings:
        match = next(
            (
                g
                for g in grandfathers
                if g.rule == finding.rule
                and g.path == finding.path
                and normalize(g.text) == normalize(finding.text)
            ),
            None,
        )
        if match is None:
            remaining.append(finding)
        else:
            applied.append(
                {
                    "rule": match.rule,
                    "path": match.path,
                    "line": finding.line,
                    "text": match.text,
                    "reason": match.reason,
                }
            )
    return remaining, applied


def run(root: pathlib.Path) -> dict:
    docs_12 = (root / DOCS_12).read_text(encoding="utf-8")
    docs_18 = (root / DOCS_18).read_text(encoding="utf-8")
    requirements = json.loads((root / REQUIREMENTS).read_text(encoding="utf-8"))

    registry = parse_registry(docs_18)
    mirror = {
        entry["id"]: entry
        for entry in requirements["requirements"]
        if entry.get("category") == "claim"
    }
    mirror_verbs = {
        entry["id"]: entry.get("summary", "")
        for entry in requirements["requirements"]
        if entry["id"] in VERB_BY_ID
    }

    findings: list[Finding] = []
    counts: dict[str, int] = {verb: 0 for verb in CONTROLLED_VERBS}
    counts["uncontrolled-candidates"] = 0
    cited: dict[str, list[tuple[str, int, str]]] = {}
    scanned: list[str] = []

    for path in scan_paths(root):
        rel = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        scanned.append(rel)
        doc_findings, doc_counts = scan_document(rel, text)
        findings.extend(doc_findings)
        for key, value in doc_counts.items():
            counts[key] = counts.get(key, 0) + value
        for block in blocks(text):
            for claim_id in CLAIM_ID.findall(block.text):
                cited.setdefault(claim_id, []).append((rel, block.line, block.text))

    references: dict[str, list[str]] = {
        claim_id: [f"{p}:{line}" for p, line, _ in sites if p != DOCS_18]
        for claim_id, sites in cited.items()
    }
    if not (root / BONES_EVENTS).is_dir():
        findings.append(
            Finding(
                rule="claim-registered-referenced",
                obligation="GOV-3-01..07",
                path=BONES_EVENTS,
                line=0,
                text=BONES_EVENTS,
                detail=(
                    "the Bones ownership ledger is absent, so claim ownership cannot be "
                    "cross-checked; run this from a full checkout"
                ),
            )
        )
    for claim_id, sources in bone_references(root).items():
        references.setdefault(claim_id, []).extend(sources)
    references = {k: v for k, v in references.items() if v}

    # §3's own text must still say what this check enforces.
    if CROSS_CHECK_SENTENCE not in section_text(docs_12, "claim governance"):
        findings.append(
            Finding(
                rule="claim-verb-registered",
                obligation="GOV-3-01..07",
                path=DOCS_12,
                line=0,
                text=CROSS_CHECK_SENTENCE,
                detail="§3 no longer states the CI cross-check obligation this check implements",
            )
        )

    if registry.states and tuple(registry.states) != DECLARED_STATES:
        findings.append(
            Finding(
                rule="claim-state-declared",
                obligation="GOV-3-01..07",
                path=DOCS_18,
                line=0,
                text=", ".join(registry.states),
                detail=(
                    "the registry's declared evidence states drifted from the closed list "
                    f"this check enforces ({', '.join(DECLARED_STATES)})"
                ),
            )
        )

    findings.extend(vocabulary_findings(parse_verb_list(docs_12), mirror_verbs))
    findings.extend(id_resolution_findings(cited, registry))
    findings.extend(registry_findings(registry, mirror, references))

    findings, grandfathered = grandfather_filter(findings, GRANDFATHERED)
    return {
        "scanned": scanned,
        "registry": registry,
        "mirror": mirror,
        "references": references,
        "findings": findings,
        "counts": counts,
        "grandfathered": grandfathered,
    }


# --- evidence ------------------------------------------------------------------

RULES_BY_OBLIGATION = "claim-verb-registered", "claim-verb-discipline"
SHARED_RULES = (
    "claim-uncontrolled-verb",
    "claim-id-resolves",
    "claim-registry-mirror",
    "claim-registered-referenced",
    "claim-state-declared",
)

BOUNDARIES: dict[str, str] = {
    "observed": (
        "Proxy. Only assertive uses (copula or perfect + `observed` in predicate "
        "position) are seen; an observation reported as a noun phrase or a bare table "
        "cell is not."
    ),
    "tested": (
        "Proxy. Only assertive uses are seen, and the corpus requirement is satisfied by "
        "any scope preposition — the check cannot tell a named corpus from a vague one."
    ),
    "bounded": (
        "Proxy. A numeral or a scope preposition counts as the bound; the check does not "
        "verify the bound is the one the evidence actually holds within."
    ),
    "exhaustively checked": (
        "Proxy. Same bound test as `bounded`. The check cannot verify exhaustiveness — "
        "only that a bound is stated alongside the word."
    ),
    "proved under": (
        "Proxy. The check verifies that assumptions are *named* after `under`; it cannot "
        "verify they are the assumptions the proof actually needs."
    ),
    "certificate checked": (
        "Proxy. The check verifies an artifact or checker is named in the sentence; it "
        "does not re-check the certificate."
    ),
    "hypothesized": (
        "Proxy. The check rejects a hypothesis stated alongside a stronger claim word; it "
        "cannot judge whether a statement should have been a hypothesis."
    ),
}

SHARED_BOUNDARY = (
    "The claim-assertion grammar is a proxy for English. It matches copula/perfect "
    "constructions with the verb in predicate position, over the scan scope only, with "
    "fenced code, inline code, and curly-quoted vocabulary removed. Claims phrased "
    "outside that grammar, or written in an RFC/ADR/research note, are not seen. "
    "`claim-registered-referenced` is satisfied by a Bone that owns the claim as well as "
    "by a document that cites it, so it proves ownership, not published citation."
)


def build_evidence(result: dict, self_test: dict) -> dict:
    registry: Registry = result["registry"]
    counts = result["counts"]
    findings: list[Finding] = result["findings"]
    by_obligation: dict[str, list[Finding]] = {}
    for finding in findings:
        by_obligation.setdefault(finding.obligation, []).append(finding)

    fixture_rules = self_test.get("fixtures", [])
    obligations: dict[str, dict] = {}
    for index, obligation in enumerate(OBLIGATION_IDS):
        verb = CONTROLLED_VERBS[index]
        caught = [
            f["fixture"]
            for f in fixture_rules
            if verb in f.get("verbs", [])
            or f.get("rule") in ("claim-verb-registered", "claim-uncontrolled-verb")
        ]
        obligations[obligation] = {
            "verb": verb,
            "requirement": {
                "source": f"{DOCS_12}:45 §3 bullet {index + 1}",
                "summary": verb,
            },
            "rules": [
                "claim-verb-registered",
                "claim-verb-discipline",
                *SHARED_RULES,
            ],
            "real_run": {
                "status": "fail" if by_obligation.get(obligation) else "pass",
                "in_scope_assertive_uses": counts.get(verb, 0),
                "discipline_exercised_on_real_corpus": counts.get(verb, 0) > 0,
                "note": (
                    ""
                    if counts.get(verb, 0)
                    else (
                        "No assertive use of this verb in the scan scope, so "
                        "claim-verb-discipline is exercised by fixture only. "
                        "claim-verb-registered and the cross-check rules bind on real "
                        "content for this ID regardless."
                    )
                ),
                "violations": [f.render() for f in by_obligation.get(obligation, [])],
            },
            "fixtures_caught": sorted(set(caught)),
            "boundary": BOUNDARIES[verb],
        }

    shared = [f for f in findings if f.obligation not in OBLIGATION_IDS]
    referenced = result["references"]
    active = [
        cid
        for cid, row in registry.rows.items()
        if result["mirror"].get(cid, {}).get("status") == "active"
    ]
    doc_referenced = sorted(
        cid
        for cid, sites in referenced.items()
        if any(not s.startswith("bones:") for s in sites)
    )
    return {
        "obligation_source": f"{DOCS_12} §3 “Claim governance”",
        "id_mapping_note": (
            "PLAN_REQUIREMENTS.json assigns GOV-3-01 … GOV-3-07 to the seven controlled "
            "verbs, one each — GOV-3-07 is “hypothesized.”, not the CI cross-check. §3's "
            "closing sentence “CI cross-checks claim IDs.” carries no requirement ID of "
            "its own, so the rules implementing it are recorded against all seven IDs and "
            "collected under `cross_check` (bn-18cg)."
        ),
        "check": "tools/governance/check_claim_governance.py",
        "controlled_verbs": list(CONTROLLED_VERBS),
        "cross_check_sentence": CROSS_CHECK_SENTENCE,
        "scan_scope": {
            "globs": list(SCAN_GLOBS),
            "documents": result["scanned"],
            "excluded": [
                "notes/plan/rfcs/",
                "notes/plan/adr/",
                "notes/plan/research/",
                "notes/plan/plan.review.*.md",
                "notes/plan/archive/",
                "notes/plan/spikes/",
                "crates/",
                "AGENTS.md",
            ],
            "excluded_within_documents": [
                "fenced code blocks",
                "inline code spans",
                "curly-quoted strings",
            ],
        },
        "claim_registry": {
            "registry": DOCS_18,
            "generated_mirror": f"{REQUIREMENTS} (category “claim”)",
            "ownership_ledger": f"{BONES_EVENTS}/*.events",
            "rows": len(registry.rows),
            "declared_states": list(registry.states),
            "active_claims": len(active),
            "referenced_claims": len(referenced),
            "referenced_by_document": doc_referenced,
            "referenced_only_by_bone": sorted(
                cid
                for cid in active
                if cid in referenced and cid not in doc_referenced
            ),
        },
        "obligations": obligations,
        "cross_check": {
            "obligation": CROSS_CHECK_SENTENCE,
            "bone": "bn-18cg",
            "recorded_against": list(OBLIGATION_IDS),
            "rules": list(SHARED_RULES),
            "real_run": {
                "status": "fail" if shared else "pass",
                "violations": [f.render() for f in shared],
            },
            "boundary": SHARED_BOUNDARY,
        },
        "counts": counts,
        "grandfathered": {
            "count": len(result["grandfathered"]),
            "instances": result["grandfathered"],
        },
        "self_test": {
            "status": self_test.get("status"),
            "fixtures": len(fixture_rules),
            "failures": self_test.get("failures", []),
        },
        "status": "fail" if findings else "pass",
    }


# --- self-test -----------------------------------------------------------------
#
# Every fixture under fixtures/claims/ declares the rule it must trigger. A
# fixture that is not caught fails the self-test, so the real run cannot pass by
# having a detector that detects nothing. `claim-clean.md` is the negative
# control: correct usage of all seven verbs must produce no finding at all.

FIXTURE_HEADER = re.compile(r"<!--\s*(?P<key>[a-z-]+):\s*(?P<value>.*?)\s*-->")


def fixture_meta(text: str) -> dict[str, str]:
    return {m.group("key"): m.group("value") for m in FIXTURE_HEADER.finditer(text)}


def self_test(root: pathlib.Path) -> dict:
    failures: list[str] = []
    results: list[dict] = []
    if not FIXTURES.is_dir():
        return {"status": "fail", "failures": [f"no fixtures under {FIXTURES}"], "fixtures": []}

    for path in sorted(FIXTURES.glob("*.md")):
        text = path.read_text(encoding="utf-8")
        meta = fixture_meta(text)
        expect = meta.get("expect-rule", "")
        kind = meta.get("kind", "document")
        rel = f"tools/governance/fixtures/claims/{path.name}"
        found: list[Finding] = []

        if kind == "document":
            found, _ = scan_document(rel, text)
        elif kind == "registry":
            registry = parse_registry(text)
            mirror = {
                cid: {
                    "id": cid,
                    "summary": row.claim,
                    "status": "active",
                    "source": {"line": row.line, "path": rel},
                }
                for cid, row in registry.rows.items()
            }
            if meta.get("mirror") == "empty":
                mirror = {}
            elif meta.get("mirror") == "orphan":
                mirror["C903"] = {
                    "id": "C903",
                    "summary": "A mirrored claim with no registry row",
                    "status": "active",
                    "source": {"line": 0, "path": REQUIREMENTS},
                }
            refs = (
                {cid: ["fixture"] for cid in registry.rows}
                if meta.get("references") == "all"
                else {}
            )
            found = registry_findings(registry, mirror, refs, registry_path=rel)
        elif kind == "id-resolution":
            registry = parse_registry((root / DOCS_18).read_text(encoding="utf-8"))
            cited: dict[str, list[tuple[str, int, str]]] = {}
            for block in blocks(text):
                for claim_id in CLAIM_ID.findall(block.text):
                    cited.setdefault(claim_id, []).append((rel, block.line, block.text))
            found = id_resolution_findings(cited, registry)
        elif kind == "vocabulary":
            verbs = parse_verb_list(text)
            mirror_verbs = {oid: VERB_BY_ID[oid] for oid in OBLIGATION_IDS}
            found = vocabulary_findings(verbs, mirror_verbs)
        else:
            failures.append(f"{path.name}: unknown fixture kind “{kind}”")
            continue

        rules = sorted({f.rule for f in found})
        if expect == "none":
            if found:
                failures.append(
                    f"{path.name}: negative control produced findings: "
                    + "; ".join(f.render() for f in found)
                )
        elif expect not in rules:
            failures.append(
                f"{path.name}: expected rule “{expect}” was not triggered (got {rules or 'nothing'})"
            )
        results.append(
            {
                "fixture": path.name,
                "kind": kind,
                "rule": expect,
                "verbs": [v for v in CONTROLLED_VERBS if v in meta.get("verbs", "")],
                "findings": [f.render() for f in found],
            }
        )

    # The grandfather list must stay honest: every entry has to still match a real
    # finding, or it is a stale exemption that hides nothing and should be deleted.
    live = run(root)
    unused = [
        g
        for g in GRANDFATHERED
        if not any(
            applied["rule"] == g.rule
            and applied["path"] == g.path
            and normalize(applied["text"]) == normalize(g.text)
            for applied in live["grandfathered"]
        )
    ]
    for stale in unused:
        failures.append(
            f"grandfather for {stale.path} ({stale.rule}) matched nothing — delete it"
        )

    return {
        "status": "fail" if failures else "pass",
        "fixtures": results,
        "failures": failures,
        "grandfathered": len(live["grandfathered"]),
    }


# --- entry point ---------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="run the fixture suite")
    parser.add_argument(
        "--evidence",
        nargs="?",
        const=str(EVIDENCE),
        default=None,
        help="write the retained evidence JSON (default: tools/governance/evidence/gov-3.json)",
    )
    parser.add_argument("--root", default=str(ROOT), help="repository root to check")
    args = parser.parse_args()
    root = pathlib.Path(args.root).resolve()

    if args.self_test and not args.evidence:
        outcome = self_test(root)
        print(json.dumps(outcome, indent=2, sort_keys=True))
        return 1 if outcome["status"] == "fail" else 0

    result = run(root)
    findings: list[Finding] = result["findings"]
    outcome = self_test(root) if args.evidence else {"status": "not-run", "fixtures": []}

    report = {
        "scanned_documents": len(result["scanned"]),
        "claim_rows": len(result["registry"].rows),
        "controlled_verb_uses": {
            verb: result["counts"].get(verb, 0) for verb in CONTROLLED_VERBS
        },
        "uncontrolled_candidates": result["counts"].get("uncontrolled-candidates", 0),
        "grandfathered": len(result["grandfathered"]),
        "grandfathered_instances": [
            f"{g['rule']} {g['path']}: {g['text']}" for g in result["grandfathered"]
        ],
        "rules": [*RULES_BY_OBLIGATION, *SHARED_RULES],
        "violations": [f.render() for f in findings],
        "status": "fail" if findings else "pass",
    }
    if args.evidence:
        report["self_test"] = outcome["status"]
        evidence = build_evidence(result, outcome)
        target = pathlib.Path(args.evidence)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        try:
            report["evidence"] = str(target.resolve().relative_to(root))
        except ValueError:
            report["evidence"] = str(target)
        if outcome["status"] == "fail":
            report["status"] = "fail"
            report["violations"] = [*report["violations"], *outcome["failures"]]

    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if report["status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
