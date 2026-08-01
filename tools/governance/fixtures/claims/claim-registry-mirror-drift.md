<!-- kind: registry -->
<!-- expect-rule: claim-registry-mirror -->
<!-- references: all -->
<!-- mirror: empty -->

# Fixture: a registry row missing from the generated mirror

Forward direction of the mirror check. `PLAN_REQUIREMENTS.json` is generated from
the registry, so a row that has no `claim` entry means the two have drifted and
every downstream label, traceability count, and coverage report is computed
against a claim set that is not the published one.

```text
HYPOTHESIS
TARGET
OBSERVED
ESTABLISHED-BOUNDED
ESTABLISHED
BLOCKED
REFUTED
```

| ID | Claim | Required evidence | Initial state |
|---|---|---|---|
| C902 | A registry row the mirror never learned about | a spike | TARGET |
