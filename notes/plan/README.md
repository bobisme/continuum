# Continuum Project Dossier — Revision 3

**Document date:** 2026-07-24  
**Status:** agent-native product architecture, executable interaction spikes, proof program, and implementation plan  
**Primary implementation language:** Rust  
**Concrete execution substrate:** asupersync  
**Proof authority:** Lean 4 `v4.32.1`  
**Compatibility corpus:** `tlaplus/Examples@91c22ea537853196ed1e03e9ad91693ec37642de`

Continuum is a verification-native environment for inventing, implementing, and maintaining concurrent and distributed Rust systems. Revision 3 makes a decisive product change:

> The verification engine is not the product by itself. The product is the shortest trustworthy path from intent to model, program, evidence, diagnosis, repair, and invention—for humans and coding agents.

Revision 2 established the independent **Model**, **Program**, and **Proof** planes. Revision 3 adds two protected layers that prevent formally sophisticated reward hacking:

```text
                              INTENT
              properties · assumptions · observers
              bounds · fault model · fairness · policy
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
                  MODEL        PROGRAM         PROOF
               mathematical   asupersync      Lean and
                semantics        Rust        certificates
                    └─────────────┼─────────────┘
                                  ▼
                               EVIDENCE
                 counterexamples · closures · receipts
                    coverage · causal explanations
                                  │
                                  ▼
                              WORKBENCH
             human IDE/CLI/TUI · agent protocol · Forge
```

Intent is immutable by default. A patch that weakens a property, strengthens an assumption, shrinks a bound, hides an event, removes a fault, or downgrades assurance is not a repair; it is a privileged intent change requiring an explicit semantic diff.

## Start here

1. [`plan.md`](plan.md) — Revision 3 master implementation plan.
2. [`docs/33_REVISION_3_ARCHITECTURE.md`](docs/33_REVISION_3_ARCHITECTURE.md) — new architecture.
3. [`docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md`](docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md) — product-level DX contract.
4. [`docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`](docs/35_CONTINUUMD_WORKBENCH_DAEMON.md) — authoritative daemon and snapshot model.
5. [`docs/41_REPAIR_TRANSACTIONS.md`](docs/41_REPAIR_TRANSACTIONS.md) — proof-gated autonomous repair.
6. [`docs/43_CONTINUUM_FORGE.md`](docs/43_CONTINUUM_FORGE.md) — verified algorithm invention.
7. [`spikes/R3_SPIKE_REPORT.md`](spikes/R3_SPIKE_REPORT.md) — executable Revision 3 findings.
8. [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md) — first 30 pull requests.
9. [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) — falsification gates.
10. [`docs/45_CONTINUUMBENCH.md`](docs/45_CONTINUUMBENCH.md) — benchmark and anti-gaming program.

## The interaction theorem

Every meaningful workbench action has the shape:

```text
(snapshot, intent, operation, budget, policy)
    → result + evidence + new snapshot or continuation
```

There is no hidden semantic session. Every long-running task is resumable by an explicit content-addressed handle. Every result names the exact semantic epoch, source snapshot, intent contract, engine, assumptions, coverage class, and certificate status.

## Primary product surfaces

```text
continuumd                 authoritative query/task/evidence daemon
continuum                  concise human CLI
cargo continuum            Rust-project entry point
Continuum LSP              authoring, semantic navigation, diagnostics
Continuum DAP              causal/time-travel verification debugger
Continuum MCP adapter      agent tools over explicit snapshot handles
Continuum SARIF exporter   CI and code-scanning diagnostics
Continuum Workbench        optional local web/TUI evidence explorer
Continuum Forge            verified synthesis and algorithm discovery
```

All surfaces are adapters over one native protocol. MCP is important interoperability, but it is not the source of truth.

## Revision 3 non-negotiables

- Agents are first-class users but never trusted authorities.
- Terminal prose is not a machine contract.
- Intent changes cannot masquerade as repairs.
- Counterexamples are causal explanations, not log dumps.
- Every context pack has an omission manifest and expansion handles.
- Every repair is a transaction with replay, neighborhood search, mutation challenge, and proof impact.
- Incremental results are provenance-bearing and differentially checked against clean builds.
- Forge optimizes only inside a fixed intent and assurance envelope.
- Humans can progressively disclose complexity without losing access to exact evidence.
- Lean remains the authority for theorems and certificate soundness; search engines remain untrusted.

## Repository map

```text
plan.md
├── docs/                  product, architecture, proof, DX, security
├── adr/                   binding architecture decisions
├── rfcs/                  implementable protocol and subsystem designs
├── research/              frontier programs with baselines and kill criteria
├── lean/                  metatheory and certificate checkers
├── schemas/               stable machine contracts
├── spikes/                executable falsification experiments
├── benchmarks/            ContinuumBench and system benchmarks
├── corpus/tla-examples/   80-family compatibility tribunal
├── examples/              human and agent walkthroughs
├── notes/                 implementation order and G0 gates
└── archive/               superseded Revision 2 entry documents
```

## Claim discipline

- `FACT` — primary source or executed evidence.
- `DESIGN` — accepted architecture.
- `HYPOTHESIS` — research claim requiring experiment.
- `TARGET` — release or implementation gate.
- `BLOCKED` — prerequisite unavailable.
- `FALSIFIED` — tested and rejected.

See [`MANIFEST.md`](MANIFEST.md), [`VALIDATION_REPORT.md`](VALIDATION_REPORT.md), and `SHA256SUMS.txt` for package integrity.
