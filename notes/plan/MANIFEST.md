# Dossier Manifest — Revision 3

**Generated:** 2026-07-24  
**Root archive name:** `continuum-project-dossier-revision-3/`  
**Integrity file:** `SHA256SUMS.txt`

## Summary

- Inventoried files, excluding this manifest and checksums: **297**
- Bytes, excluding this manifest and checksums: **1,174,266**
- Approximate Markdown words: **112,721**
- ADRs: **52**
- RFCs: **40**
- Research programs: **36**
- Lean source modules: **17**

### By extension

| Extension | Files |
|---|---:|
| `.csv` | 2 |
| `.ctm` | 4 |
| `.json` | 38 |
| `.lean` | 17 |
| `.md` | 211 |
| `.py` | 18 |
| `.rs` | 1 |
| `.toml` | 5 |
| `[none]` | 1 |

### By top-level area

| Area | Files |
|---|---:|
| `.` | 6 |
| `adr` | 53 |
| `archive` | 3 |
| `benchmarks` | 6 |
| `corpus` | 16 |
| `docs` | 56 |
| `examples` | 7 |
| `lean` | 20 |
| `notes` | 2 |
| `research` | 37 |
| `rfcs` | 41 |
| `schemas` | 30 |
| `spikes` | 18 |
| `tools` | 2 |

## Complete inventory

### `.`

- `README.md` — 6,514 bytes — Continuum Project Dossier — Revision 3
- `REVISION_3_CHANGELOG.md` — 3,101 bytes — Revision 3 Changelog
- `VALIDATION_REPORT.md` — 4,736 bytes — Dossier Validation Report — Revision 3
- `plan.md` — 72,725 bytes — Continuum: Revision 3 Master Implementation Plan
- `plan.review.1.md` — 73,838 bytes — Review of `plan.md` (Revision 3 Master Implementation Plan) and the accompanying dossier
- `validation-results.json` — 947 bytes
### `adr`

- `adr/0001-asupersync-execution-substrate.md` — 2,009 bytes — ADR-0001: Use asupersync as the execution substrate
- `adr/0002-causal-intermediate-representation.md` — 1,646 bytes — ADR-0002: Adopt a causal intermediate representation as the semantic interchange
- `adr/0003-no-ambient-nondeterminism.md` — 1,311 bytes — ADR-0003: Prohibit ambient nondeterminism in verified cores
- `adr/0004-standalone-models-and-zoomable-refinement.md` — 1,315 bytes — ADR-0004: Support standalone models and zoomable refinement views
- `adr/0005-partial-order-primary-semantics.md` — 1,294 bytes — ADR-0005: Make partial-order executions primary
- `adr/0006-typed-assurance-lattice.md` — 1,079 bytes — ADR-0006: Represent assurance as a multidimensional typed claim
- `adr/0007-verification-engine-portfolio.md` — 1,291 bytes — ADR-0007: Use a portfolio of verification engines over one semantics
- `adr/0008-proof-carrying-results.md` — 1,191 bytes — ADR-0008: Require checkable certificates for strong success claims
- `adr/0009-production-partial-order-conformance.md` — 1,271 bytes — ADR-0009: Validate production traces as partial orders
- `adr/0010-cancellation-calculus.md` — 1,205 bytes — ADR-0010: Make cancellation and obligations part of formal semantics
- `adr/0011-semantic-domain-packs.md` — 950 bytes — ADR-0011: Represent external systems as versioned semantic domain packs
- `adr/0012-dual-authoring-surfaces.md` — 1,118 bytes — ADR-0012: Provide a standalone model language and a Rust integration surface
- `adr/0013-exact-state-identity.md` — 1,189 bytes — ADR-0013: Use exact canonical state identity in exhaustive and certified modes
- `adr/0014-explicit-fairness-and-ranking-liveness.md` — 1,097 bytes — ADR-0014: Use explicit fairness plus automata and ranking-function liveness lanes
- `adr/0015-strong-refinement-for-hyperproperties.md` — 1,168 bytes — ADR-0015: Distinguish ordinary refinement from hyperproperty-preserving refinement
- `adr/0016-typed-timed-and-probabilistic-extensions.md` — 1,092 bytes — ADR-0016: Add timed and probabilistic semantics as typed extensions
- `adr/0017-solver-and-tcb-policy.md` — 977 bytes — ADR-0017: Treat solvers as accelerators unless their evidence is checked
- `adr/0018-semantic-versioning-and-replay.md` — 968 bytes — ADR-0018: Version semantics independently from APIs and artifact formats
- `adr/0019-adapter-isolation.md` — 984 bytes — ADR-0019: Isolate runtime/compiler/foreign-tool adapters from semantic core
- `adr/0020-experimental-mathematics-governance.md` — 1,085 bytes — ADR-0020: Gate speculative mathematics behind falsifiable research programs
- `adr/0021-tla-examples-corpus-contract.md` — 2,666 bytes — ADR-0021: Make the TLA+ Examples corpus a release contract
- `adr/0022-lean4-metatheory-and-certificate-authority.md` — 2,970 bytes — ADR-0022: Use Lean 4 as metatheory and certificate authority
- `adr/0023-semantic-triptych-and-independent-paths.md` — 2,204 bytes — ADR-0023: Maintain a semantic triptych with independent paths
- `adr/0024-observer-indexed-independence.md` — 2,105 bytes — ADR-0024: Index independence by observation and property contracts
- `adr/0025-stratified-model-language-fragments.md` — 1,944 bytes — ADR-0025: Stratify the model language into declared semantic fragments
- `adr/0026-proof-producing-reductions.md` — 1,787 bytes — ADR-0026: Require proof-producing reductions for strong assurance
- `adr/0027-nominal-orbit-finite-symmetry.md` — 1,836 bytes — ADR-0027: Add a nominal/orbit-finite lane for names and symmetry
- `adr/0028-assumption-synthesis-as-games.md` — 1,728 bytes — ADR-0028: Treat environment and fairness assumption synthesis as games
- `adr/0029-semantic-equivalence-not-tla-source-cloning.md` — 1,227 bytes — ADR-0029: Target semantic equivalence, not a TLA+ source clone
- `adr/0030-semiring-valued-analysis.md` — 1,731 bytes — ADR-0030: Generalize exploration results with typed semiring analyses
- `adr/0031-corpus-derived-language-governance.md` — 1,031 bytes — ADR 0031: Corpus-Derived Language Governance
- `adr/0032-weak-memory-execution-graph-lane.md` — 944 bytes — ADR 0032: Weak Memory Is a Separate Execution-Graph Lane
- `adr/0033-checked-projection-and-generated-rust-interfaces.md` — 844 bytes — ADR 0033: Checked Projection May Generate Rust Interfaces
- `adr/0034-owned-temporal-semantics-with-leanltl-bridge.md` — 801 bytes — ADR 0034: Own the Temporal Semantics and Bridge to LeanLTL
- `adr/0035-proof-receipts-and-axiom-manifests.md` — 810 bytes — ADR 0035: Strong Claims Emit Proof Receipts and Axiom Manifests
- `adr/0036-authoritative-workbench-daemon.md` — 937 bytes — ADR 0036: Authoritative Workbench Daemon
- `adr/0037-explicit-content-addressed-handles.md` — 878 bytes — ADR 0037: Explicit Content-Addressed Handles
- `adr/0038-machine-contracts-not-terminal-prose.md` — 742 bytes — ADR 0038: Machine Contracts, Not Terminal Prose
- `adr/0039-protected-intent-contract.md` — 834 bytes — ADR 0039: Protected Intent Contract
- `adr/0040-typed-context-packs.md` — 763 bytes — ADR 0040: Typed Context Packs
- `adr/0041-verification-debugger-and-dap.md` — 788 bytes — ADR 0041: Verification Debugger with DAP Projection
- `adr/0042-mcp-adapter-not-authority.md` — 728 bytes — ADR 0042: MCP Is an Adapter, Not Authority
- `adr/0043-repair-transactions.md` — 713 bytes — ADR 0043: Repair Transactions
- `adr/0044-incremental-semantic-database.md` — 748 bytes — ADR 0044: Incremental Semantic Database with Clean Tribunal
- `adr/0045-progressive-disclosure-with-lossless-expansion.md` — 648 bytes — ADR 0045: Progressive Disclosure with Lossless Expansion
- `adr/0046-semantic-diff-gates.md` — 757 bytes — ADR 0046: Semantic and Intent Diff Gates
- `adr/0047-forge-sandbox-and-nonvacuity.md` — 740 bytes — ADR 0047: Forge Sandboxing and Non-Vacuity
- `adr/0048-proof-service-isolation.md` — 755 bytes — ADR 0048: Proof Service Isolation
- `adr/0049-explicit-budgets-and-resumable-search.md` — 736 bytes — ADR 0049: Explicit Budgets and Resumable Search
- `adr/0050-proof-oriented-model-program-lenses.md` — 783 bytes — ADR 0050: Proof-Oriented Model/Program Lenses
- `adr/0051-assurance-envelope.md` — 659 bytes — ADR 0051: Assurance Envelope Instead of Verified Badge
- `adr/0052-multi-agent-evidence-graph.md` — 662 bytes — ADR 0052: Multi-Agent Evidence Graph
- `adr/README.md` — 5,596 bytes — Architecture Decision Records
### `archive`

- `archive/README-revision-2.md` — 6,383 bytes — Continuum Project Dossier — Revision 2
- `archive/REVISION_2_CHANGELOG.md` — 3,405 bytes — Revision 2 Changelog
- `archive/plan-revision-2.md` — 68,889 bytes — Continuum: Master Implementation Plan
### `benchmarks`

- `benchmarks/README.md` — 2,755 bytes — Continuum Evaluation Corpus
- `benchmarks/benchmark-manifest.example.toml` — 765 bytes
- `benchmarks/continuum-bench/README.md` — 1,008 bytes — ContinuumBench Seed
- `benchmarks/continuum-bench/tasks/governance-property-weakening.json` — 583 bytes
- `benchmarks/continuum-bench/tasks/repair-ack-before-durable.json` — 651 bytes
- `benchmarks/continuum-bench/tasks/synthesize-ack-guard.json` — 623 bytes
### `corpus`

- `corpus/tla-examples/CORPUS_POLICY.md` — 2,500 bytes — Corpus Governance Policy
- `corpus/tla-examples/FEATURE_CENSUS.md` — 3,515 bytes — Semantic Feature Census Forced by the Corpus
- `corpus/tla-examples/PARITY_LEVELS.md` — 3,637 bytes — TLA+ Corpus Parity Levels
- `corpus/tla-examples/PORTING_WAVES.md` — 3,381 bytes — Porting Waves
- `corpus/tla-examples/README.md` — 2,291 bytes — TLA+ Examples Compatibility Corpus
- `corpus/tla-examples/check_inventory.py` — 2,211 bytes
- `corpus/tla-examples/corpus-summary.json` — 1,824 bytes
- `corpus/tla-examples/ingest_local_clone.py` — 2,621 bytes
- `corpus/tla-examples/other-examples.csv` — 3,303 bytes
- `corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm` — 819 bytes
- `corpus/tla-examples/ports/TV-007/README.md` — 457 bytes — TV-007 — Dining Philosophers
- `corpus/tla-examples/ports/TV-009/DieHard.ctm` — 971 bytes
- `corpus/tla-examples/ports/TV-009/README.md` — 490 bytes — TV-009 — Die Hard
- `corpus/tla-examples/ports/TV-009/default.model.toml` — 262 bytes
- `corpus/tla-examples/ports/TV-009/port.json` — 1,760 bytes
- `corpus/tla-examples/validated-examples.csv` — 13,454 bytes
### `docs`

- `docs/00_EXECUTIVE_SPEC.md` — 6,112 bytes — Continuum Executive Specification — Revision 2
- `docs/01_ARCHITECTURE.md` — 6,635 bytes — Continuum Architecture — Revision 2
- `docs/02_SEMANTICS.md` — 7,464 bytes — Semantic Contract
- `docs/03_ASSURANCE_AND_TCB.md` — 7,928 bytes — Assurance Model and Trusted Computing Base
- `docs/04_IMPLEMENTATION_ROADMAP.md` — 3,880 bytes — Implementation Roadmap — Revision 2
- `docs/05_RESEARCH_LANDSCAPE.md` — 12,556 bytes — Research Landscape and Gap Analysis
- `docs/06_RESEARCH_AGENDA.md` — 13,666 bytes — Frontier Research Agenda
- `docs/07_BENCHMARKS_AND_EVALUATION.md` — 7,100 bytes — Benchmarks and Evaluation
- `docs/08_RISK_REGISTER.md` — 7,343 bytes — Risk Register and Kill Criteria
- `docs/09_THREAT_MODEL.md` — 6,854 bytes — Threat Model
- `docs/10_INTEROP_AND_MIGRATION.md` — 4,662 bytes — Interoperability and Migration Strategy
- `docs/11_LANGUAGE_AND_DX.md` — 5,401 bytes — Model Language and Developer Experience
- `docs/12_GOVERNANCE_AND_ENGINEERING.md` — 3,913 bytes — Governance and Engineering Discipline
- `docs/13_BIBLIOGRAPHY.md` — 31,824 bytes — Primary-Source Bibliography
- `docs/14_GLOSSARY.md` — 3,649 bytes — Glossary
- `docs/15_COMPETITIVE_MATRIX.md` — 2,781 bytes — Competitive Matrix
- `docs/16_PROOF_OBLIGATIONS.md` — 5,531 bytes — Proof Obligation Catalog
- `docs/17_DOMAIN_PACK_CONTRACT.md` — 4,472 bytes — Domain Pack Contract
- `docs/18_CLAIMS_MATRIX.md` — 5,217 bytes — Claims Matrix
- `docs/19_TEST_STRATEGY.md` — 3,982 bytes — Test and Validation Strategy
- `docs/20_ADVERSARIAL_DESIGN_REVIEW.md` — 9,141 bytes — Adversarial Design Review
- `docs/21_NOVEL_CONTRIBUTIONS.md` — 6,014 bytes — Candidate Novel Contributions
- `docs/22_TLA_EXAMPLES_COMPATIBILITY_PROGRAM.md` — 5,365 bytes — TLA+ Examples Compatibility Program
- `docs/23_LEAN4_FORMALIZATION_PROGRAM.md` — 5,660 bytes — Lean 4 Formalization Program
- `docs/24_REVISION_2_ARCHITECTURE.md` — 4,455 bytes — Revision-2 Architecture: The Semantic Triptych
- `docs/25_SEMANTIC_FEATURE_MATRIX.md` — 3,134 bytes — Semantic Feature and Backend Matrix
- `docs/26_RELEASE_GATES_REV2.md` — 3,145 bytes — Revision-2 Program Gates
- `docs/27_SPIKE_FINDINGS_REV2.md` — 2,861 bytes — Revision-2 Spike Findings
- `docs/28_THEORY_OF_SUCCESS.md` — 4,146 bytes — Theory of Success
- `docs/29_PROOF_ENGINEERING_STRATEGY.md` — 3,027 bytes — Proof Engineering Strategy
- `docs/30_AGENT_NATIVE_MICRO_REVOLUTION.md` — 2,875 bytes — Agent-Native Concurrent Systems Engineering
- `docs/31_FALSIFICATION_AND_KILL_CRITERIA.md` — 3,346 bytes — Falsification and Kill Criteria
- `docs/32_CORPUS_PORTING_PLAYBOOK.md` — 2,435 bytes — Corpus Porting Playbook
- `docs/33_REVISION_3_ARCHITECTURE.md` — 6,225 bytes — Revision 3 Architecture: Intent, Evidence, and Workbench
- `docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md` — 4,820 bytes — Developer Experience Product Contract
- `docs/35_CONTINUUMD_WORKBENCH_DAEMON.md` — 4,687 bytes — `continuumd`: Authoritative Workbench Daemon
- `docs/36_AGENT_PROTOCOL_AND_TOOL_CONTRACTS.md` — 3,669 bytes — Agent Protocol and Tool Contracts
- `docs/37_HUMAN_WORKFLOWS.md` — 3,544 bytes — Human Workflows
- `docs/38_COUNTEREXAMPLE_EXPERIENCE.md` — 3,324 bytes — Counterexample Experience
- `docs/39_VERIFICATION_DEBUGGER.md` — 4,193 bytes — Verification Debugger
- `docs/40_SEMANTIC_DIFF_AND_IMPACT.md` — 3,229 bytes — Semantic Diff and Impact Analysis
- `docs/41_REPAIR_TRANSACTIONS.md` — 3,522 bytes — Repair Transactions
- `docs/42_INCREMENTAL_VERIFICATION.md` — 3,255 bytes — Incremental Verification
- `docs/43_CONTINUUM_FORGE.md` — 4,848 bytes — Continuum Forge: Verified Algorithm Invention
- `docs/44_MULTI_AGENT_EVIDENCE_GRAPH.md` — 3,323 bytes — Multi-Agent Evidence Graph
- `docs/45_CONTINUUMBENCH.md` — 4,204 bytes — ContinuumBench
- `docs/46_IDE_CLI_DAP_MCP_AND_SARIF.md` — 2,869 bytes — IDE, CLI, DAP, MCP, and SARIF Integration
- `docs/47_ONBOARDING_AND_PROGRESSIVE_DISCLOSURE.md` — 3,121 bytes — Onboarding and Progressive Disclosure
- `docs/48_EXPLANATION_SCIENCE.md` — 2,970 bytes — Explanation Science Program
- `docs/49_SECURITY_FOR_AUTONOMOUS_AGENTS.md` — 3,309 bytes — Security for Autonomous Agents
- `docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md` — 2,883 bytes — Agent Evaluation and Reward Hacking
- `docs/51_PRODUCT_WALKTHROUGHS.md` — 3,802 bytes — Product Walkthroughs
- `docs/52_RELEASE_GATES_REV3.md` — 3,252 bytes — Revision 3 Release Gates
- `docs/53_SPIKE_FINDINGS_REV3.md` — 5,952 bytes — Revision 3 Executable Spike Report
- `docs/54_THEORY_OF_AGENTIC_ACCELERATION.md` — 3,759 bytes — Theory of Agentic Acceleration
- `docs/55_AGENT_API_REFERENCE_SKETCH.md` — 3,915 bytes — Agent API Reference Sketch
### `examples`

- `examples/agent_workflow.md` — 1,060 bytes — Agent Workflow Example: Durable Acknowledgement Repair
- `examples/continuum.project.toml` — 380 bytes
- `examples/forge_ack_protocol.ctm` — 783 bytes
- `examples/pseudo_api.rs` — 2,716 bytes
- `examples/replicated_register.ctm` — 1,304 bytes
- `examples/replicated_register.md` — 3,590 bytes — First Vertical Slice: Cancel-Correct Replicated Register
- `examples/replicated_register.scenario.toml` — 702 bytes
### `lean`

- `lean/Continuum.lean` — 512 bytes
- `lean/Continuum/Cancellation.lean` — 828 bytes
- `lean/Continuum/Certificate.lean` — 1,014 bytes
- `lean/Continuum/EventStructure.lean` — 1,084 bytes
- `lean/Continuum/Examples/DieHard.lean` — 788 bytes
- `lean/Continuum/Fairness.lean` — 1,233 bytes
- `lean/Continuum/Independence.lean` — 1,101 bytes
- `lean/Continuum/Interaction/ContextSlice.lean` — 1,129 bytes
- `lean/Continuum/Interaction/Incremental.lean` — 1,128 bytes
- `lean/Continuum/Interaction/Intent.lean` — 1,488 bytes
- `lean/Continuum/Interaction/Repair.lean` — 1,359 bytes
- `lean/Continuum/Interaction/Synthesis.lean` — 1,349 bytes
- `lean/Continuum/ProofReceipt.lean` — 701 bytes
- `lean/Continuum/Refinement.lean` — 2,741 bytes
- `lean/Continuum/Semantics.lean` — 1,043 bytes
- `lean/Continuum/Symmetry.lean` — 909 bytes
- `lean/Continuum/Temporal.lean` — 899 bytes
- `lean/README.md` — 2,707 bytes — Continuum Lean Metatheory Seed
- `lean/lakefile.toml` — 112 bytes
- `lean/lean-toolchain` — 25 bytes
### `notes`

- `notes/G0_SPIKE_MATRIX.md` — 3,886 bytes — G0 Falsification Matrix — Revision 3
- `notes/START_HERE_IMPLEMENTATION.md` — 11,346 bytes — Start Here: Revision 3 Implementation Sequence
### `research`

- `research/01-true-concurrency.md` — 6,640 bytes — Research Note 01: True Concurrency as the Native Verification Object
- `research/02-symbolic-inductive-and-parameterized.md` — 6,539 bytes — Research Note 02: Symbolic, Inductive, and Parameterized Verification
- `research/03-refinement-composition-and-sheaves.md` — 5,981 bytes — Research Note 03: Refinement, Composition, and Local-to-Global Reasoning
- `research/04-liveness-progress-and-fairness.md` — 4,813 bytes — Research Note 04: Liveness, Progress, and Fairness at Scale
- `research/05-runtime-conformance-and-observability.md` — 4,750 bytes — Research Note 05: Runtime Conformance Under Partial Observation
- `research/06-timed-probabilistic-and-hyperproperties.md` — 4,008 bytes — Research Note 06: Timed, Probabilistic, and Hyperproperty Extensions
- `research/07-alien-mathematics.md` — 7,325 bytes — Research Note 07: Alien-Mathematics Portfolio
- `research/08-agentic-verification.md` — 4,188 bytes — Research Note 08: Proof-Gated Agentic Verification
- `research/09-cancellation-and-obligation-calculus.md` — 4,205 bytes — Research Note 09: Cancellation and Obligation Calculus
- `research/10-combinatorial-geometry-of-concurrency.md` — 7,530 bytes — Research Note 10: Combinatorial Geometry of Concurrency
- `research/11-tla-examples-corpus-gap-analysis.md` — 4,472 bytes — Research 11: TLA+ Examples Corpus Gap Analysis
- `research/12-lean-reflection-and-proof-certificates.md` — 3,422 bytes — Research 12: Lean Reflection and Proof Certificates
- `research/13-observer-indexed-independence.md` — 3,067 bytes — Research 13: Observer-Indexed Independence
- `research/14-nominal-sets-and-orbit-finite-verification.md` — 3,084 bytes — Research 14: Nominal Sets and Orbit-Finite Verification
- `research/15-games-and-assumption-synthesis.md` — 3,151 bytes — Research 15: Games and Assumption Synthesis
- `research/16-causal-abstract-interpretation.md` — 2,794 bytes — Research 16: Causal Abstract Interpretation
- `research/17-proof-producing-reductions-and-translation-validation.md` — 2,549 bytes — Research 17: Proof-Producing Reductions and Translation Validation
- `research/18-coalgebra-modal-logic-and-behavioral-interfaces.md` — 2,539 bytes — Research 18: Coalgebra, Modal Logic, and Behavioral Interfaces
- `research/19-semiring-provenance-and-weighted-exploration.md` — 2,287 bytes — Research 19: Semiring Provenance and Weighted Exploration
- `research/20-rust-concurrency-verification-frontier-2026.md` — 2,857 bytes — Research 20: Rust Concurrency Verification Frontier, 2026
- `research/21-choreographies-and-session-types.md` — 3,743 bytes — Choreographies, Multiparty Session Types, and Verified Projection
- `research/22-weak-memory-execution-graphs.md` — 2,927 bytes — Weak Memory as Execution Graphs
- `research/23-compositional-proof-algebra.md` — 2,778 bytes — A Compositional Algebra of Models, Refinements, and Certificates
- `research/24-automatic-abstraction-and-property-directed-modeling.md` — 2,984 bytes — Automatic Abstraction and Property-Directed Modeling
- `research/25-agent-computer-interfaces-for-formal-systems.md` — 4,003 bytes — Agent–Computer Interfaces for Formal Systems
- `research/26-causal-and-contrastive-counterexample-explanations.md` — 3,876 bytes — Causal and Contrastive Counterexample Explanations
- `research/27-incremental-verification-and-proof-reuse.md` — 3,320 bytes — Incremental Verification and Proof Reuse
- `research/28-agentic-proof-repair-and-context-compilation.md` — 3,103 bytes — Agentic Proof Repair and Context Compilation
- `research/29-protocol-invariant-ranking-and-abstraction-cosynthesis.md` — 2,968 bytes — Protocol, Invariant, Ranking, and Abstraction Co-Synthesis
- `research/30-quality-diversity-for-verified-algorithm-discovery.md` — 2,470 bytes — Quality-Diversity for Verified Algorithm Discovery
- `research/31-proof-oriented-bidirectional-transformations.md` — 3,092 bytes — Proof-Oriented Bidirectional Transformations
- `research/32-semantic-context-compilation-and-information-theory.md` — 2,751 bytes — Semantic Context Compilation and Information Theory
- `research/33-agent-benchmarks-and-reward-hacking.md` — 2,396 bytes — Agent Benchmarks and Formal Reward Hacking
- `research/34-human-factors-of-formal-systems-workbenches.md` — 2,606 bytes — Human Factors of Formal-Systems Workbenches
- `research/35-security-of-agentic-verification-workbenches.md` — 2,351 bytes — Security of Agentic Verification Workbenches
- `research/36-active-diagnosis-and-experiment-design.md` — 2,503 bytes — Active Diagnosis and Experiment Design
- `research/README.md` — 4,424 bytes — Research Program
### `rfcs`

- `rfcs/0001-causal-intermediate-representation.md` — 8,384 bytes — RFC 0001: Causal Intermediate Representation
- `rfcs/0002-controlled-effects-and-domain-packs.md` — 5,843 bytes — RFC 0002: Controlled Effects and Semantic Domain Packs
- `rfcs/0003-continuum-model-language.md` — 6,126 bytes — RFC 0003: Continuum Model Language
- `rfcs/0004-exploration-dpor-and-unfoldings.md` — 5,584 bytes — RFC 0004: Exploration, DPOR, and Unfoldings
- `rfcs/0005-certificates-and-independent-kernel.md` — 4,805 bytes — RFC 0005: Certificates and the Independent Kernel
- `rfcs/0006-production-trace-conformance.md` — 4,111 bytes — RFC 0006: Production Partial-Order Conformance
- `rfcs/0007-storage-and-crash-semantics.md` — 3,563 bytes — RFC 0007: Storage, Crash, and Recovery Semantics
- `rfcs/0008-liveness-fairness-and-progress.md` — 3,743 bytes — RFC 0008: Liveness, Fairness, and Progress
- `rfcs/0009-views-and-refinement.md` — 4,085 bytes — RFC 0009: Zoomable Views and Refinement
- `rfcs/0010-assurance-results-and-claims.md` — 3,184 bytes — RFC 0010: Assurance Results and Claims
- `rfcs/0011-tla-examples-tribunal.md` — 4,674 bytes — RFC 0011: TLA+ Examples Compatibility Tribunal
- `rfcs/0012-lean-metatheory-and-reflective-certificates.md` — 4,889 bytes — RFC 0012: Lean Metatheory and Reflective Certificates
- `rfcs/0013-semantic-triptych.md` — 2,916 bytes — RFC 0013: Semantic Triptych and Cross-Path Validation
- `rfcs/0014-observer-indexed-dpor.md` — 3,537 bytes — RFC 0014: Observer-Indexed DPOR and Causal Reduction
- `rfcs/0015-temporal-fairness-and-hyperproperties.md` — 3,073 bytes — RFC 0015: Temporal, Fairness, and Hyperproperty Semantics
- `rfcs/0016-parameterized-and-nominal-verification.md` — 2,565 bytes — RFC 0016: Parameterized, Symmetric, and Nominal Verification
- `rfcs/0017-assumption-and-protocol-synthesis-games.md` — 2,466 bytes — RFC 0017: Assumption and Protocol Synthesis as Games
- `rfcs/0018-agent-native-proof-and-repair-loop.md` — 2,341 bytes — RFC 0018: Agent-Native Proof and Repair Loop
- `rfcs/0019-corpus-port-manifest.md` — 1,880 bytes — RFC 0019: Corpus Port Manifest and Evidence Schema
- `rfcs/0020-proof-producing-transformations.md` — 2,355 bytes — RFC 0020: Proof-Producing Transformations and Optimization Pipeline
- `rfcs/0021-corpus-oracle-protocol.md` — 921 bytes — RFC 0021: Corpus Oracle Protocol
- `rfcs/0022-weak-memory-local-refinement.md` — 749 bytes — RFC 0022: Weak-Memory Local Refinement
- `rfcs/0023-checked-choreographic-projection.md` — 703 bytes — RFC 0023: Checked Choreographic Projection
- `rfcs/0024-proof-receipt-format.md` — 695 bytes — RFC 0024: Proof Receipt Format
- `rfcs/0025-property-directed-abstraction-loop.md` — 694 bytes — RFC 0025: Property-Directed Abstraction Loop
- `rfcs/0026-continuumd-native-protocol.md` — 6,771 bytes — RFC 0026: `continuumd` Native Protocol
- `rfcs/0027-agent-tool-protocol.md` — 4,920 bytes — RFC 0027: Agent Tool Protocol
- `rfcs/0028-context-pack-format.md` — 5,207 bytes — RFC 0028: Context Pack Format and Compiler
- `rfcs/0029-causal-verification-debugger.md` — 808 bytes — RFC 0029: Causal Verification Debugger
- `rfcs/0030-incremental-semantic-query-engine.md` — 5,661 bytes — RFC 0030: Incremental Semantic Query Engine
- `rfcs/0031-semantic-and-intent-diff.md` — 5,716 bytes — RFC 0031: Semantic and Intent Diff
- `rfcs/0032-repair-transaction-protocol.md` — 6,178 bytes — RFC 0032: Repair Transaction Protocol
- `rfcs/0033-continuum-forge.md` — 985 bytes — RFC 0033: Continuum Forge
- `rfcs/0034-continuum-bench.md` — 811 bytes — RFC 0034: ContinuumBench Task and Grader Contract
- `rfcs/0035-isolated-lean-proof-service.md` — 812 bytes — RFC 0035: Isolated Lean Proof Service
- `rfcs/0036-proof-oriented-correspondence.md` — 772 bytes — RFC 0036: Proof-Oriented Model/Program Correspondence
- `rfcs/0037-intent-contract.md` — 7,136 bytes — RFC 0037: Intent Contract Schema and Policy
- `rfcs/0038-multi-agent-evidence-graph.md` — 728 bytes — RFC 0038: Multi-Agent Evidence Graph
- `rfcs/0039-explanation-engine.md` — 663 bytes — RFC 0039: Explanation Engine
- `rfcs/0040-protocol-adapters.md` — 713 bytes — RFC 0040: LSP, DAP, MCP, SARIF, and CLI Adapters
- `rfcs/README.md` — 3,519 bytes — Requests for Comments
### `schemas`

- `schemas/assurance-result.schema.json` — 5,339 bytes
- `schemas/benchmark-task.schema.json` — 1,938 bytes
- `schemas/cir.schema.json` — 11,792 bytes
- `schemas/context-pack.schema.json` — 6,224 bytes
- `schemas/corpus-port.schema.json` — 4,098 bytes
- `schemas/crashpack.schema.json` — 3,848 bytes
- `schemas/domain-pack.schema.json` — 4,674 bytes
- `schemas/evidence-graph-node.schema.json` — 2,032 bytes
- `schemas/examples/benchmark-task.example.json` — 612 bytes
- `schemas/examples/context-pack.example.json` — 1,425 bytes
- `schemas/examples/counterexample.crashpack.json` — 1,178 bytes
- `schemas/examples/diehard.corpus-port.json` — 1,760 bytes
- `schemas/examples/evidence-graph-node.example.json` — 392 bytes
- `schemas/examples/finite-closure.proof-receipt.json` — 1,365 bytes
- `schemas/examples/finite-proof.assurance.json` — 1,681 bytes
- `schemas/examples/intent-contract.example.json` — 1,997 bytes
- `schemas/examples/minimal.cir.json` — 1,102 bytes
- `schemas/examples/repair-transaction.example.json` — 898 bytes
- `schemas/examples/semantic-diff.example.json` — 754 bytes
- `schemas/examples/storage-pack.manifest.json` — 1,698 bytes
- `schemas/examples/synthesis-candidate.example.json` — 563 bytes
- `schemas/examples/verification-task.example.json` — 532 bytes
- `schemas/examples/workspace-snapshot.example.json` — 521 bytes
- `schemas/intent-contract.schema.json` — 9,406 bytes
- `schemas/proof-receipt.schema.json` — 3,943 bytes
- `schemas/repair-transaction.schema.json` — 4,880 bytes
- `schemas/semantic-diff.schema.json` — 3,891 bytes
- `schemas/synthesis-candidate.schema.json` — 2,761 bytes
- `schemas/verification-task.schema.json` — 2,753 bytes
- `schemas/workspace-snapshot.schema.json` — 2,301 bytes
### `spikes`

- `spikes/R3_SPIKE_REPORT.md` — 5,932 bytes — Revision 3 Executable Spike Report
- `spikes/SPIKE_REPORT.md` — 4,228 bytes — Revision-2 Executable Spike Report
- `spikes/advanced_spikes.py` — 14,731 bytes
- `spikes/agent_context_spike.py` — 3,737 bytes
- `spikes/agent_protocol_spike.py` — 2,588 bytes
- `spikes/causal_debugger_spike.py` — 1,887 bytes
- `spikes/evidence_graph_spike.py` — 8,823 bytes
- `spikes/forge_cegis_spike.py` — 1,762 bytes
- `spikes/incremental_verification_spike.py` — 2,504 bytes
- `spikes/models.py` — 6,220 bytes
- `spikes/observer_independence.py` — 2,232 bytes
- `spikes/proof_lens_spike.py` — 1,701 bytes
- `spikes/reference_kernel.py` — 5,476 bytes
- `spikes/results/r3-spike-results.json` — 12,758 bytes
- `spikes/results/spike-results.json` — 10,863 bytes
- `spikes/run_r3_spikes.py` — 1,000 bytes
- `spikes/run_spikes.py` — 7,843 bytes
- `spikes/semantic_diff_spike.py` — 4,091 bytes
### `tools`

- `tools/generate_manifest.py` — 3,785 bytes
- `tools/validate_dossier.py` — 11,045 bytes
