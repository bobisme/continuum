<!-- kind: registry -->
<!-- expect-rule: claim-registry-mirror -->
<!-- references: all -->
<!-- mirror: orphan -->

# Fixture: a mirrored claim with no registry row

Reverse direction of the mirror check. The generated mirror carries `C903`, which
appears in no claim row: a claim that requirements, Bones, and coverage reports
can all cite while the published registry never states it.

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
| C904 | An ordinary registered claim | a spike | TARGET |
