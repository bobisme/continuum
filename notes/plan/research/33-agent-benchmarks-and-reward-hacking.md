# Agent Benchmarks and Formal Reward Hacking

**Claim class:** evaluation methodology

## Lane status (plan §24.5)

This note owns two register rows:

1. **Neighborhood adequacy (§8.3)** — lane OPEN and ratified. Threshold: hidden-variant catch rate of the §8.3 repair neighborhood on the docs/50-classified gaming corpus, fixed numerically in *Lane opening: ratified neighborhood adequacy threshold* below (at least 20 mutations per docs/50 attack class; at least 90 percent caught overall). The kill clause is the first kill criterion below (hidden variants too easy to leak or too hard to grade independently); the fallback stays the fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim.
2. **Co-ownership of the context-compilation ablation benchmark** (general context compilation, §6) with research/25 and research/32: this note supplies the anti-gaming grading discipline for that lane's agent benchmark.

## Problem

Standard coding benchmarks often grade final tests. Formal-systems agents can exploit the specification, environment, bounds, observer, verifier, or grader itself.

## Threat model

The agent may knowingly or accidentally:

- alter intent;
- exploit visible cases;
- disable instrumentation;
- submit stale evidence;
- consume excessive resources;
- manipulate tool state;
- inject instructions via source;
- overfit proof/search to known lemmas.

## Benchmark principles

1. Protected Intent Contract outside writable snapshot.
2. Hidden semantic variants, not only hidden tests.
3. Independent grader/checker.
4. Full operation/evidence trace.
5. Resource-normalized reporting.
6. Family-level split and semantic clone detection.
7. Security/capability tasks.
8. Exact artifact requirement.

## Novel proposal: adversarial intent mutations

For every task, automatically derive neighboring intents:

- remove property conjunct;
- add assumption;
- reduce bound;
- remove fault;
- coarsen observer;
- strengthen fairness;
- downgrade assurance.

The agent must preserve the original fingerprint. The benchmark checks whether its patch accidentally behaves as if it solved a neighboring easier intent.

## Novel proposal: verifier-aware hidden variants

Generate variants preserving high-level intent but changing:

- names/types/order;
- topology size within cutoff;
- equivalent action decomposition;
- scheduler/fault realization;
- implementation structure;
- proof lemma names;
- domain profile.

This measures semantic generalization.

## Novel proposal: trajectory evidence score

A successful final patch can still arise from unsafe behavior. Score:

- invalid privileged attempts;
- unsupported claims;
- stale handle use;
- context expansion efficiency;
- hypotheses refuted/updated;
- evidence status honesty;
- cancellation/budget behavior.

Trajectory score is diagnostic; final semantic evidence remains primary.

## Baselines

- raw shell coding agent;
- ACI agent;
- ACI + Context Packs;
- ACI + multi-agent evidence graph;
- human engineer;
- human-agent team.

## Lane opening: ratified neighborhood adequacy threshold (plan §24.5, FR-05)

The lane is opened here and its numbers are fixed here; plan §24.5 quotes the
sentence verbatim under the same `quote-id`.

quote-id=neighborhood-adequacy-catch-rate "On the docs/50-classified gaming corpus — at least 20 hidden mutations for each of the five docs/50 attack classes (intent, instrumentation, evidence, overfitting, resource), so at least 100 mutations in total — the §8.3 neighborhood is adequate only if it catches at least 90 percent of the mutations overall with the one-sided 95 percent exact binomial lower bound on that rate above 80 percent and no single attack class below 75 percent, counting a mutation as caught only when property-directed neighboring exploration surfaces a neighbor on which the gaming patch fails and the receipt discloses that neighbor, with every mutation drawn from a family-level held-out split and graded by the independent grader of research/33."

Rationale. The per-class count is derived from docs/50, not chosen freely:
docs/50 defines exactly five attack classes and names 25 concrete techniques
inside them (intent 6, instrumentation 5, evidence 5, overfitting 5, resource
4), so a corpus that samples a class only once per technique cannot separate a
neighborhood that catches a technique from one that caught a single lucky
instance of it. Applying the repeated-seeds principle of this note — the same
discipline the ratified research/25 margin fixes at three seeds — gives at
least three independent realizations per named technique, which is 18 for the
largest class; 20 is that number rounded to a uniform per-class floor, so the
six-technique intent class still carries three realizations each and the
four-technique resource class carries five. A uniform floor rather than a
total corpus size is what stops the cheapest class from being over-sampled to
inflate a global rate. Five classes at 20 puts the corpus at at least 100
graded mutations, which is also the smallest size at which the catch rate
carries usable power: a neighborhood whose true adequacy is only 80 percent
reaches 90 out of 100 by chance about 0.6 percent of the time, while a
neighborhood that is genuinely 95 percent adequate clears the same bar about
99 percent of the time, so the target neither passes a weak neighborhood nor
punishes a good one for sampling noise. The 90 percent point target and the
80 percent interval floor are jointly satisfiable rather than contradictory:
at exactly 90 of 100 the one-sided 95 percent exact (Clopper-Pearson) lower
bound is 0.836, so the point estimate is the binding constraint and the
interval only rules out a corpus small enough for 90 percent to be an
accident. The target is deliberately not set at 95 percent: discriminating a
95 percent claim from a 90 percent one at the same confidence needs several
hundred independently graded mutations, and grading cost is exactly what this
note's first kill criterion warns about, so a higher headline would buy
precision by making the lane unfalsifiable in practice. The 75 percent
per-class floor (15 of 20) is a screen, not a powered test: it exists so a
strong average cannot hide one blind class, and at 20 mutations a class whose
true adequacy is 60 percent fails the floor about 87 percent of the time while
a class at 90 percent passes it about 99 percent of the time.

The counting rule and the split follow §8.3 and the benchmark principles above.
A catch counts only when property-directed neighboring exploration surfaces a
neighbor on which the gaming patch fails **and** the receipt discloses that
neighbor, because §8.3 already requires coverage to appear in the receipt and
an undisclosed catch cannot be audited by a reviewer or distinguished from a
post-hoc claim. The family-level held-out split and the independent grader are
principles 2, 3, and 6 of this note applied to the adequacy measurement itself;
they are also the instruments for the kill criterion, since a leaked or
grader-dependent variant set shows up as a catch rate that moves when the split
or the grader changes.

Scope and dependency. The docs/50-classified gaming corpus does not exist
today; it is a Phase B G3 deliverable, and docs/50 supplies the attack-class
taxonomy the corpus must be classified against, not the corpus. Fixing the
numbers now is the point: the corpus is built to the ratified size and the
neighborhood is measured against a target that was not fitted to the corpus
after the fact. Until the corpus lands and is graded, the §8.3 neighborhood
makes no adequacy claim and the register fallback stands — a fixed
strategy-list neighborhood with per-receipt coverage disclosure.

## Kill criteria

- hidden variants are too easy to leak or too hard to grade independently;
- benchmark success fails to predict real project adoption;
- cost normalization dominates task semantics;
- task generation introduces invalid or ambiguous intents.
