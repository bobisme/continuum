# RFC 0024: Proof Receipt Format

## Summary

Define the content-addressed record for a strongest-assurance Continuum result.

## Fields

- receipt schema version;
- claim and assurance class;
- model/property/assumption/observer hashes;
- semantic, corpus, domain-pack, and proof epochs;
- transformation steps and preservation classes;
- certificate type/hash;
- checker source/binary hash;
- Lean version, theorem names, imports, and axiom manifest;
- foreign evidence references, if any;
- reproduction command;
- optional signatures.

## Validation

Schema validation is necessary but insufficient. `continuum proof check` recomputes hashes and invokes the appropriate independent checker.
