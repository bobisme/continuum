# Research Landscape and Gap Analysis

This document summarizes the relevant state of the art as of 2026-07-24. Source IDs refer to [`13_BIBLIOGRAPHY.md`](13_BIBLIOGRAPHY.md).

## 1. Abstract protocol modeling

### TLA+ and TLC

TLA+ remains exceptional for unconstrained state-machine abstraction, safety/liveness, fairness, and mathematical state. TLC provides explicit-state exploration but carries installation and implementation friction and maintains a hard model/code boundary. Continuum should preserve TLA+'s freedom to choose abstraction, not merely its syntax. [S01][S64]

### Quint and Apalache

Quint modernizes authoring with types, a REPL, simulation, and model-based testing, while Apalache lowers TLA+-style models to SMT and supports bounded checking and inductiveness. They validate the value of a typed frontend and a layered IR. Continuum should interoperate rather than fork users unnecessarily. [S02][S03][S04]

### P

P's event-driven machines, monitors, systematic testing, inductive verifier, and runtime observation come closest to lifecycle integration. Its architectural commitment to machines/events is useful but narrower than arbitrary mathematical abstraction. [S05]

**Gap:** none of these systems make a production Rust runtime's cancellation, durability, task ownership, and causal events native to the same semantics as the abstract model.

## 2. Deterministic simulation and systematic concurrency

### FoundationDB and TigerBeetle

Both demonstrate the decisive engineering pattern: run production code in a deterministic environment with virtualized nondeterminism, accelerate time, inject faults, and preserve exact seeds. This is the strongest empirical evidence for replacing bespoke DSTs with a common substrate. [S06][S07]

### Loom, Shuttle, and GenMC

Loom instruments Rust synchronization but does not model the full C11 memory model and is intentionally local. Shuttle offers randomized/replay/PCT-style schedule exploration. GenMC performs stateless model checking over LLVM memory models. [S08][S09][S43]

### MODIST and related transparent checkers

MODIST showed that interposing on an unmodified distributed system can expose actions to a centralized deterministic explorer and found many protocol bugs. The cost is semantic opacity and platform-specific interposition. [S12]

**Gap:** local concurrency, distributed faults, cancellation, and production semantics are typically controlled by different tools.

## 3. Model/code conformance

### Quint Connect

Quint Connect generates model traces and runs them against Rust implementations. This validates model-based testing as a practical bridge. [S04]

### OmniLink

OmniLink maps black-box concurrent events with timeboxes into TLA+ meanings and solves for a legal total order. It demonstrates that useful validation need not require a fabricated observed total order and can find previously unknown bugs. [S13]

### Multi-grained ZooKeeper verification

Recent work on ZooKeeper uses fine and coarse TLA+ component models together, addressing the tradeoff between state explosion and model/code gaps and finding severe bugs. [S14]

**Gap:** conformance remains a post hoc workflow. Continuum makes multi-grain views and partial-order trace completion native.

## 4. Partial-order reduction and true concurrency

### DPOR

Classic DPOR, optimal DPOR, observer variants, and parsimonious optimal DPOR provide increasingly precise exploration of Mazurkiewicz trace classes. They are the practical baseline. [S15][S16][S17]

### Petri-net unfoldings

Complete finite prefixes represent concurrent executions without enumerating all interleavings. Recent work extends symbolic prefixes to high-level nets and infinite-marking classes. Modular unfoldings summarize component behavior through interfaces. [S21][S22A]

### Higher-dimensional automata

Recent Kleene, Myhill–Nerode, and logical characterization results give a serious formal-language theory for interval pomsets and non-interleaving concurrency. This is not yet a production model-checking recipe, but it offers tools for canonicalization and compositional languages beyond traces. [S22][S23][S24A]

### Directed topology

Spaces of directed executions and homotopy classes formalize families of schedules. They may support reduction and visualization, but practical value must be benchmarked. [S24B]

**Gap:** practical verification rarely treats partial-order objects as the stable interchange artifact.

## 5. High-performance state exploration

### LTSmin/PINS

LTSmin separates modeling languages from verification algorithms using the Partitioned Next-State Interface. It combines explicit, symbolic, parallel, LTL, and POR algorithms. This strongly supports Continuum's engine-independent semantic interface. [S18]

### Sylvan and saturation

Parallel decision diagrams and saturation can outperform flat exploration on asynchronous systems by exploiting transition locality. They must be portfolio engines, because representation performance is highly model-dependent. [S19][S20]

**Gap:** a modern Rust-native system could combine partial-order events, PINS-style action groups, exact state encodings, and proof evidence, but should reuse mature algorithmic ideas rather than assume a concurrent hash set is enough.

## 6. Inductive and parameterized verification

### PDR/IC3 and CHCs

IC3/PDR learns inductive invariants through property-directed blocking. Spacer generalizes this style to infinite-state systems and CHCs; global-guidance work addresses local-generalization instability. [S26][S27]

### IC3PO and Ivy

IC3PO connects symmetry with quantification and cutoff discovery, automatically deriving quantified invariants for protocols including Paxos. Ivy restricts models into decidable fragments to automate parameterized proofs. [S28][S29]

### Well-structured transition systems

WSTS theory gives decidable coverability for monotone infinite-state systems using well-quasi-orders and ideal completions. It is powerful for counters, multisets, lossy channels, and broadcast-like systems, but not a general distributed verifier. [S61]

**Gap:** users manually choose and reformulate systems for each proof technology. Continuum can detect structure and lower one semantic model into restricted lanes with explicit applicability conditions.

## 7. Liveness

Finite-state liveness can use SCC/automata methods, but parameterized distributed liveness remains difficult. LVR reduces many protocol liveness proofs to safety using ranking functions; newer relational-ranking work aims at scalable progress proofs. [S30][S31]

**Gap:** deterministic simulators usually test finite progress, while model checkers separate fairness from concrete runtime cancellation and fault assumptions. Continuum can connect fairness and rankings to asupersync's structural lifecycle.

## 8. Refinement and verified distributed systems

IronFleet and Verdi established end-to-end or framework-based verified distributed implementations. Aneris, Grove, Perennial, and Trillium provide expressive separation logics and refinement frameworks for concurrency, distributed systems, crashes, and TLA+-style models. [S32]–[S37]

These systems demonstrate that layered refinement is the right theoretical shape. Their proof-engineering cost and language/tool separation remain barriers for routine systems development.

**Gap:** automated, executable, multi-grained refinement tied to ordinary Rust workflows.

## 9. Rust verification

Verus, Creusot, Kani, Aeneas, hax, Crux, RustBelt, and GenMC cover complementary regions:

- deductive functional correctness;
- bit-precise bounded code checking;
- translation to proof assistants;
- unsafe-code semantics;
- weak-memory exploration;
- machine-checked foundations. [S38]–[S45]

Interior mutability and concurrency remain difficult; library-defined capabilities and hybrid safe/unsafe verification are active research.

**Gap:** code verifiers do not replace an abstract distributed protocol model. Continuum should export local obligations and use these tools below the protocol-refinement boundary.

## 10. Certificates

Alethe provides an SMT proof format; Carcara is a Rust checker. LRAT has efficient verified checkers. Model-checking certificates exist for finite state, probabilistic systems, and interactive certification. [S47]–[S49]

**Gap:** mainstream distributed-system model checking rarely makes certifying success a first-class developer artifact.

## 11. Timed and probabilistic systems

UPPAAL, IMITATOR, Storm, and PRISM provide mature specialized engines for timed, parametric timed, probabilistic, and stochastic systems. Rare-event simulation and statistical model checking address failures too rare for naive sampling. [S50]–[S52][S56]

**Gap:** these semantics are typically separate models. Continuum should add them as typed domain/property extensions without weakening the deterministic core.

## 12. Hyperproperties and strong refinement

Ordinary refinement may not preserve distributions or security hyperproperties. Strong observational refinement and progressive forward simulations characterize stronger preservation conditions. HyperLTL tools automate selected hyperproperties. [S53][S54][S55]

**Gap:** system verifiers routinely claim refinement without stating which property classes are preserved.

## 13. Composition, effects, and choreographies

Algebraic/higher-order effects provide modular interpretations of operations. Choreographic programming and multiparty session types can project global protocols to local endpoints with safety/liveness properties, including refined and timed protocols. [S58]–[S60]

**Opportunity:** Continuum's effect packs and views can learn from these systems, while avoiding the claim that every distributed implementation should be generated from one global choreography.

## 14. Synthesis and agents

SyGuS/CEGIS, reactive synthesis, and AI-assisted Verus work show that invariants, functions, and proofs can be generated effectively in constrained domains. Every useful result still depends on a mechanical verifier. [S57][S69][S70]

**Gap:** agents receive poor semantic feedback. Continuum's causal counterexamples and explicit proof obligations can make agentic repair much more reliable.


## 15. 2025–2026 frontier delta

Several recent results sharpen the architecture:

- QSM-Cutoff systematically derives quantified cutoff formulas from symmetry-aware finite reachability, strengthening the case for an orbit-lifted parameterized lane rather than naïve finite extrapolation. [S77]
- Await-aware optimal DPOR removes semantically pure waiting executions and weakens conflict where justified; Continuum's runtime semantics should expose await/quiescence structure rather than treating every poll as an event. [S78]
- Asynchronous fault-tolerant runtime-verification results establish decisive monitorability limits for real-time-order-sensitive properties, requiring `Inconclusive` as a first-class production verdict. [S79]
- Algorithm-selection results in hardware verification support a measured portfolio scheduler, but only as a performance layer over independently sound engines. [S80]
- RustMC demonstrates stateless checking of compiled Rust/FFI code through GenMC, reinforcing the value of an adapter lane below Continuum's protocol semantics. [S81]
- Partial-order conformance through Petri-net unfoldings provides a concrete baseline for Continuum's production alignment engine. [S82]
- LRAT-Catcher shows that large external solver certificates can be imported compositionally into Lean through reflection, strengthening the long-term path from Continuum certificates to theorem-prover artifacts. [S84]
- Thrust and Gillian-Rust show rapid progress in CHC/refinement-type automation and hybrid safe/unsafe Rust verification; Continuum should generate local proof obligations rather than attempting to subsume these tools. [S71][S86]

## 16. Overall conclusion

No existing system combines all of the following as one semantic product:

- arbitrary standalone abstract models;
- real Rust/asupersync execution;
- cancel-correct effect semantics;
- deterministic distributed simulation;
- partial-order-first exploration;
- zoomable refinement;
- inductive/liveness lanes;
- production partial-order conformance;
- typed assurance;
- independently checked certificates.

The project is plausible because nearly every ingredient has independent evidence. The risk lies in semantic integration and scope, not in the absence of algorithms.
