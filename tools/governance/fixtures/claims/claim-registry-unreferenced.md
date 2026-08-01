<!-- kind: registry -->
<!-- expect-rule: claim-registered-referenced -->
<!-- references: none -->

# Fixture: a registered claim nobody references

The reverse direction of the cross-check. A claim row that no scanned document
cites and no Bone owns is an orphan: it can never be promoted, refuted, or
retired, because nothing points at it.

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
| C900 | An orphan claim referenced by nothing | none recorded | TARGET |
