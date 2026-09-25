# Architecture Decision Records

ADRs are normative unless superseded.

| ADR | Decision |
|---|---|
| [0001](0001-asupersync-execution-substrate.md) | Use asupersync as the execution substrate |
| [0002](0002-causal-intermediate-representation.md) | Adopt a causal intermediate representation as the semantic interchange |
| [0003](0003-no-ambient-nondeterminism.md) | Prohibit ambient nondeterminism in verified cores |
| [0004](0004-standalone-models-and-zoomable-refinement.md) | Support standalone models and zoomable refinement views |
| [0005](0005-partial-order-primary-semantics.md) | Make partial-order executions primary |
| [0006](0006-typed-assurance-lattice.md) | Represent assurance as a multidimensional typed claim |
| [0007](0007-verification-engine-portfolio.md) | Use a portfolio of verification engines over one semantics |
| [0008](0008-proof-carrying-results.md) | Require checkable certificates for strong success claims |
| [0009](0009-production-partial-order-conformance.md) | Validate production traces as partial orders |
| [0010](0010-cancellation-calculus.md) | Make cancellation and obligations part of formal semantics |
| [0011](0011-semantic-domain-packs.md) | Represent external systems as versioned semantic domain packs |
| [0012](0012-dual-authoring-surfaces.md) | Provide a standalone model language and a Rust integration surface |
| [0013](0013-exact-state-identity.md) | Use exact canonical state identity in exhaustive and certified modes |
| [0014](0014-explicit-fairness-and-ranking-liveness.md) | Use explicit fairness plus automata and ranking-function liveness lanes |
| [0015](0015-strong-refinement-for-hyperproperties.md) | Distinguish ordinary refinement from hyperproperty-preserving refinement |
| [0016](0016-typed-timed-and-probabilistic-extensions.md) | Add timed and probabilistic semantics as typed extensions |
| [0017](0017-solver-and-tcb-policy.md) | Treat solvers as accelerators unless their evidence is checked |
| [0018](0018-semantic-versioning-and-replay.md) | Version semantics independently from APIs and artifact formats |
| [0019](0019-adapter-isolation.md) | Isolate runtime/compiler/foreign-tool adapters from semantic core |
| [0020](0020-experimental-mathematics-governance.md) | Gate speculative mathematics behind falsifiable research programs |
| [0021](0021-tla-examples-corpus-contract.md) | Make the TLA+ Examples corpus a release contract |
| [0022](0022-lean4-metatheory-and-certificate-authority.md) | Use Lean 4 as metatheory and certificate authority |
| [0023](0023-semantic-triptych-and-independent-paths.md) | Maintain a semantic triptych with independent paths |
| [0024](0024-observer-indexed-independence.md) | Index independence by observation and property contracts |
| [0025](0025-stratified-model-language-fragments.md) | Stratify the model language into declared semantic fragments |
| [0026](0026-proof-producing-reductions.md) | Require proof-producing reductions for strong assurance |
| [0027](0027-nominal-orbit-finite-symmetry.md) | Add a nominal/orbit-finite lane for names and symmetry |
| [0028](0028-assumption-synthesis-as-games.md) | Treat environment and fairness assumption synthesis as games |
| [0029](0029-semantic-equivalence-not-tla-source-cloning.md) | Target semantic equivalence, not a TLA+ source clone |
| [0030](0030-semiring-valued-analysis.md) | Generalize exploration results with typed semiring analyses |
| [0031](0031-corpus-derived-language-governance.md) | Corpus-Derived Language Governance |
| [0032](0032-weak-memory-execution-graph-lane.md) | Weak Memory Is a Separate Execution-Graph Lane |
| [0033](0033-checked-projection-and-generated-rust-interfaces.md) | Checked Projection May Generate Rust Interfaces |
| [0034](0034-owned-temporal-semantics-with-leanltl-bridge.md) | Own the Temporal Semantics and Bridge to LeanLTL |
| [0035](0035-proof-receipts-and-axiom-manifests.md) | Strong Claims Emit Proof Receipts and Axiom Manifests |

## Revision 3 decisions

| ADR | Decision |
|---|---|
| [0036](0036-authoritative-workbench-daemon.md) | Authoritative Workbench Daemon |
| [0037](0037-explicit-content-addressed-handles.md) | Explicit Content-Addressed Handles |
| [0038](0038-machine-contracts-not-terminal-prose.md) | Machine Contracts, Not Terminal Prose |
| [0039](0039-protected-intent-contract.md) | Protected Intent Contract |
| [0040](0040-typed-context-packs.md) | Typed Context Packs |
| [0041](0041-verification-debugger-and-dap.md) | Verification Debugger with DAP Projection |
| [0042](0042-mcp-adapter-not-authority.md) | MCP Is an Adapter, Not Authority |
| [0043](0043-repair-transactions.md) | Repair Transactions |
| [0044](0044-incremental-semantic-database.md) | Incremental Semantic Database with Incremental Parity Audit |
| [0045](0045-progressive-disclosure-with-lossless-expansion.md) | Progressive Disclosure with Lossless Expansion |
| [0046](0046-semantic-diff-gates.md) | Semantic and Intent Diff Gates |
| [0047](0047-forge-sandbox-and-nonvacuity.md) | Forge Sandboxing and Non-Vacuity |
| [0048](0048-proof-service-isolation.md) | Proof Service Isolation |
| [0049](0049-explicit-budgets-and-resumable-search.md) | Explicit Budgets and Resumable Search |
| [0050](0050-proof-oriented-model-program-lenses.md) | Proof-Oriented Model/Program Lenses |
| [0051](0051-assurance-envelope.md) | Assurance Envelope Instead of Verified Badge |
| [0052](0052-multi-agent-evidence-graph.md) | Multi-Agent Evidence Graph |

## Program decisions

| ADR | Decision |
|---|---|
| [0053](0053-product-license-mit-or-apache-2.md) | Product license is `MIT OR Apache-2.0`; the asupersync rider is accepted and disclosed |
| [0054](0054-signing-scheme.md) | Signing identities use Ed25519 through `ed25519-dalek`, over canonical bytes, with content-addressed signers |
| [0055](0055-incremental-engine-build-vs-adopt.md) | The §9 incremental engine is custom; salsa is not adopted; five measured reversal criteria |

New ADRs use the next number and include context, decision, consequences, evidence requirements, and supersession rules.
