<!-- kind: registry -->
<!-- expect-rule: claim-state-declared -->
<!-- references: all -->

# Fixture: a claim row in an undeclared evidence state

The state lattice is closed the same way the verb set is. A row in a state the
registry never declared cannot be mapped to allowed wording, so docs/18's
state→wording rule has nothing to bind.

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
| C901 | A claim in a state the registry never declared | a spike | NEARLY-ESTABLISHED |
