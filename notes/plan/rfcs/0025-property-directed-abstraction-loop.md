# RFC 0025: Property-Directed Abstraction Loop

## Summary

Define a CEGAR-style workflow for proposing and validating abstraction maps from concrete CIR/Rust systems to CML models.

## Loop

1. derive conservative dependency slice from property/observer;
2. propose predicates and abstract fields;
3. construct abstract transformer;
4. check property/refinement;
5. classify counterexample as concrete or spurious;
6. refine predicates/history/prophecy state;
7. emit semantic diff and obligations;
8. promote only with bounded or theorem receipt.

## Agent role

Agents may propose predicates and mappings. They cannot mark a spurious trace or accept an abstraction without checker evidence.
