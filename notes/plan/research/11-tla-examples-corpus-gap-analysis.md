# Research 11: TLA+ Examples Corpus Gap Analysis

## Research question

What capabilities must Continuum possess before it can honestly replace the practical role of TLA+ across a heterogeneous example corpus?

## Corpus role

The TLA+ Examples repository describes itself as a comprehensive example library, a corpus for tool development/testing, and a case-study collection. Its manifests and configs explicitly exercise:

- PlusCal and proofs;
- action composition;
- exhaustive, simulation, trace generation and symbolic modes;
- safety, liveness, deadlock and assumption failures;
- `SYMMETRY`, `VIEW`, `ALIAS`, `CONSTRAINT`, and `DEADLOCK` configuration features.

Source:
https://github.com/tlaplus/Examples

## Gap 1 — mathematical value semantics

Rust-oriented model DSLs often start with structs, enums and collections. TLA+ examples routinely treat sets and total functions as first-class mathematical values, quantify over them, construct powersets, and use model values independent of implementation representation.

Continuum response:

- persistent canonical value algebra;
- finite and symbolic capabilities;
- row-polymorphic records and tagged sums;
- functions represented extensionally in finite lanes;
- explicit definedness;
- typed escape for heterogeneous mathematical data where needed.

Risk: an overly strict type system can silently remove legitimate behaviors and make parity appear easier.

## Gap 2 — action formulas, not commands

Primed variables, existential next-state values, `UNCHANGED`, enabledness, and stuttering are relational. An imperative DSL can encode them only with care.

Continuum response:

- relational core;
- procedural surface lowered with proof-producing transformation;
- frame inference checked against explicit write sets;
- first-class action labels for fairness/refinement.

## Gap 3 — infinite behaviors

A DST explores finite runs. TLA+ specs denote infinite behaviors, often closed under stuttering. Fairness changes which infinite behaviors count.

Continuum response:

- behavior semantics independent of the runtime;
- explicit terminal completion policy;
- Büchi/Streett checking and ranking proofs;
- fairness assumptions in result identity;
- Lean semantics for preservation.

## Gap 4 — model configuration is semantic

Constants, overrides, constraints, symmetry, views and deadlock settings materially change a model. Treating config as CLI decoration makes results unreproducible.

Continuum response:

- typed, hashed model configuration;
- proof-producing finite instantiation;
- view/alias as observation contracts;
- model constraints separated from specification assumptions;
- exact config in every evidence artifact.

## Gap 5 — refinement

Several corpus cases are explicitly about refinement, auxiliary variables or multi-grain specifications. Trace equality is inadequate.

Continuum response:

- relational/stuttering refinement edges;
- history/prophecy variables;
- observer/event maps;
- composition theorem;
- strong refinement lane for hyperproperties.

## Gap 6 — theorem proving

TLAPS-bearing examples contain claims beyond finite model checking. A Rust checker alone cannot replace that role.

Continuum response:

- Lean theorem parity;
- finite certificates imported by reflection;
- reusable protocol mathematics;
- no line-by-line TLAPS emulation requirement.

## Gap 7 — source language metafeatures

Modules, instantiation, recursive operators, higher-order operators, level checking and nested definitions appear in corpus stress cases.

Continuum response:

- predictable typed module system;
- no unrestricted host macros in trusted elaboration;
- semantic feature waves;
- optional TLA frontend after native semantics stabilize.

## Gap 8 — domain diversity

The corpus covers puzzles, shared memory, distributed protocols, cache coherence, storage, TCP, files, B-trees, probability and cyber-physical scheduling.

Continuum response:

- domain packs only for concrete/runtime semantics;
- abstract models remain domain-independent mathematics;
- specialized timed/probabilistic fragments rather than forcing everything through generic events.

## Conclusion

Corpus parity changes the target from “excellent Rust DST” to “verification environment with DST as one interpretation.” The largest technical risks are temporal semantics, theorem/refinement parity, and maintaining a rich mathematical language without losing predictable execution.
