# Primary-Source Bibliography

**Cutoff:** sources reviewed through 2026-07-24.  
This is curated for architectural relevance, not exhaustive. Preprints are labeled.

## A. TLA+, Quint, P, and protocol-modeling ecosystems

### [S01] TLA+

Leslie Lamport, TLA+ home and materials.  
https://lamport.azurewebsites.net/tla/tla.html

### [S02] Apalache

Apalache symbolic model checker, official site and documentation.  
https://apalache-mc.org/  
https://github.com/apalache-mc/apalache

### [S03] Quint

Quint specification language, official documentation and repository.  
https://quint-lang.org/  
https://github.com/informalsystems/quint

### [S04] Quint Connect

“Quint Connect: Model-Based Testing for Real Implementations,” 2025; Emerald case study, 2026.  
https://quint-lang.org/posts/quint_connect  
https://quint-lang.org/posts/quint_connect_emerald  
https://github.com/informalsystems/quint-connect

### [S05] P

P programming language and verification system, official documentation.  
https://p-org.github.io/P/

### [S64] TLC architecture

TLA+ tools codebase architecture.  
https://docs.tlapl.us/codebase%3Aarchitecture  
https://github.com/tlaplus/tlaplus

### [S65] Quint transpiler architecture

Quint ADR: transpiler architecture and layered IR.  
https://quint-lang.org/docs/development-docs/architecture-decision-records/adr001-transpiler-architecture

### [S67] PObserve

P runtime monitoring/observation materials.  
https://p-org.github.io/P/

## B. Deterministic simulation and implementation checking

### [S06] FoundationDB simulation

FoundationDB testing and engineering documentation.  
https://apple.github.io/foundationdb/testing.html  
https://apple.github.io/foundationdb/engineering.html

### [S07] TigerBeetle VOPR and architecture

TigerBeetle architecture and deterministic simulation.  
https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/ARCHITECTURE.md

### [S08] Loom

Tokio Loom repository and documentation.  
https://github.com/tokio-rs/loom  
https://docs.rs/loom/latest/loom/

### [S09] Shuttle

Shuttle deterministic concurrency testing.  
https://github.com/awslabs/shuttle  
https://docs.rs/shuttle/latest/shuttle/

### [S10] Asupersync

Asupersync repository and README.  
https://github.com/Dicklesworthstone/asupersync

### [S11] Asupersync Lab/trace/DPOR notes

Asupersync reference document for Lab, traces, and DPOR.  
https://github.com/Dicklesworthstone/asupersync/blob/main/skills/asupersync-mega-skill/references/LAB-TRACE-DPOR.md

### [S12] MODIST

Junfeng Yang et al., “MODIST: Transparent Model Checking of Unmodified Distributed Systems,” NSDI 2009.  
https://www.microsoft.com/en-us/research/publication/modist-transparent-model-checking-of-unmodified-distributed-systems/

### [S13] OmniLink

Finn Hackett et al., “Trace Validation of Unmodified Concurrent Systems with OmniLink,” preprint, 2026.  
https://arxiv.org/abs/2601.11836

### [S14] Multi-grained ZooKeeper specifications

Lingzhi Ouyang et al., “Multi-Grained Specifications for Distributed System Model Checking and Verification,” EuroSys 2025.  
https://doi.org/10.1145/3689031.3696069  
https://arxiv.org/abs/2409.14301

### [S68] TLA+ trace validation

Horatiu Cirstea et al., “Validating Traces of Distributed Programs Against TLA+ Specifications,” SEFM 2024.  
https://doi.org/10.1007/978-3-031-77382-2_8  
https://arxiv.org/abs/2404.16075

## C. Partial-order reduction, unfoldings, and true concurrency

### [S15] Dynamic Partial-Order Reduction

Cormac Flanagan and Patrice Godefroid, “Dynamic Partial-Order Reduction for Model Checking Software,” POPL 2005.  
https://doi.org/10.1145/1040305.1040315

### [S16] Optimal DPOR

Parosh Aziz Abdulla et al., “Optimal Dynamic Partial Order Reduction,” POPL 2014.  
https://doi.org/10.1145/2535838.2535845

### [S17] Parsimonious Optimal DPOR

Parosh Aziz Abdulla et al., “Parsimonious Optimal Dynamic Partial Order Reduction,” CAV 2024.  
https://doi.org/10.1007/978-3-031-65630-9_2  
https://arxiv.org/abs/2405.11128

### [S21] Symbolic complete finite prefixes

Nick Würdemann et al., “Taking Complete Finite Prefixes To High Level, Symbolically,” Fundamenta Informaticae, 2024.  
https://doi.org/10.3233/FI-242196

### [S22A] Modular finite complete prefixes

Agnes Madalinski and Eric Fabre, “Modular Construction of Finite and Complete Prefixes of Petri Net Unfoldings,” 2009.  
https://doi.org/10.3233/FI-2009-148

### [S22] Kleene theorem for higher-dimensional automata

Uli Fahrenberg et al., “Kleene Theorem for Higher-Dimensional Automata,” LMCS 2024.  
https://doi.org/10.46298/lmcs-20(4:22)2024  
https://arxiv.org/abs/2202.03791

### [S23] Myhill–Nerode theorem for HDA

Uli Fahrenberg and Krzysztof Ziemiański, “Myhill-Nerode Theorem for Higher-Dimensional Automata,” 2024.  
https://doi.org/10.3233/FI-242194

### [S24A] Logic and languages of HDA

Amazigh Amrane et al., “Logic and Languages of Higher-Dimensional Automata,” preprint, 2024.  
https://arxiv.org/abs/2403.19526

### [S24B] Directed paths in HDA

Martin Raussen, “Strictifying and Taming Directed Paths in Higher Dimensional Automata,” 2021.  
https://doi.org/10.1017/S0960129521000280

### [S24] Sheaf-theoretic distributed tasks

Stephan Felber, Bernardo Hummes Flores, Hugo Rincon Galeana, “A Sheaf-Theoretic Characterization of Tasks in Distributed Systems,” preprint, 2025.  
https://arxiv.org/abs/2503.02556

## D. High-performance model checking

### [S18] LTSmin

Gijs Kant et al., “LTSmin: High-Performance Language-Independent Model Checking,” TACAS 2015.  
https://doi.org/10.1007/978-3-662-46681-0_61

### [S19] Sylvan

Tom van Dijk and Jaco van de Pol, “Sylvan: Multi-Core Decision Diagrams,” TACAS 2015; extended STTT paper.  
https://doi.org/10.1007/978-3-662-46681-0_60  
https://doi.org/10.1007/s10009-016-0433-2

### [S20] Saturation

Gianfranco Ciardo, Gerald Lüttgen, Radu Siminiceanu, “Saturation: An Efficient Iteration Strategy for Symbolic State-Space Generation,” 2001.  
https://ntrs.nasa.gov/citations/20010022506

### [S62] PINS architecture

See LTSmin paper and associated PINS materials.  
https://www.tvandijk.nl/publication/kantlmpbd15/

### [S71A] Recomposition

Ian Dardik, April Porter, Eunsuk Kang, “Recomposition: A New Technique for Efficient Compositional Verification,” preprint, 2024.  
https://arxiv.org/abs/2408.03488

## E. Abstract interpretation, induction, parameterization, and infinite state

### [S25] Abstract interpretation

Patrick Cousot and Radhia Cousot, “Abstract Interpretation: A Unified Lattice Model for Static Analysis,” POPL 1977.  
https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml

### [S26] IC3/PDR

Aaron R. Bradley, “SAT-Based Model Checking without Unrolling,” VMCAI 2011.  
https://doi.org/10.1007/978-3-642-18275-4_7

### [S27] Spacer/global guidance

Hari Govind Vediramana Krishnan et al., “Global Guidance for Local Generalization in Model Checking,” Formal Methods in System Design, 2024.  
https://doi.org/10.1007/s10703-023-00412-3

### [S28] IC3PO

Aman Goel and Karem Sakallah, “On Symmetry and Quantification: A New Approach to Verify Distributed Protocols,” 2021.  
https://arxiv.org/abs/2103.14831

### [S28A] Automatic Paxos invariant

Aman Goel and Karem Sakallah, “Towards an Automatic Proof of Lamport's Paxos,” 2021.  
https://arxiv.org/abs/2108.08796

### [S29] Ivy

Microsoft Research Ivy project and CAV tool paper.  
https://www.microsoft.com/en-us/research/project/ivy/

### [S61] Well-structured transition systems

Alain Finkel and Philippe Schnoebelen, WSTS foundations and later ideal-theory work.  
https://doi.org/10.1016/0890-5401(90)90009-7  
https://doi.org/10.1016/j.ic.2020.104582

### [S71] Thrust

Hiromi Ogawa, Taro Sekiyama, Hiroshi Unno, “Thrust: A Prophecy-Based Refinement Type System for Rust,” PLDI 2025.  
https://doi.org/10.1145/3729333

## F. Liveness and temporal verification

### [S30] LVR

Jianan Yao et al., “Mostly Automated Verification of Liveness Properties for Distributed Protocols with Ranking Functions,” POPL 2024.  
https://doi.org/10.1145/3632877

### [S31] Toward Liveness Proofs at Scale

Kenneth L. McMillan, CAV 2024.  
https://doi.org/10.1007/978-3-031-65627-9_13

### [S31A] Incremental progress model checking

Aaron R. Bradley et al., “An Incremental Approach to Model Checking Progress Properties,” FMCAD 2011.  
https://plv.colorado.edu/papers/fair-fmcad11.html

### [S55] AutoHyper

Raven Beutner and Bernd Finkbeiner, AutoHyper for full HyperLTL.  
https://autohyper.github.io/  
https://doi.org/10.1007/978-3-031-30823-9_8  
https://doi.org/10.1007/s10009-025-00801-5

## G. Refinement and verified distributed systems

### [S32] IronFleet

Chris Hawblitzel et al., “IronFleet: Proving Practical Distributed Systems Correct,” SOSP 2015.  
https://www.microsoft.com/en-us/research/project/ironclad/publications/

### [S33] Verdi

James R. Wilcox et al., “Verdi: A Framework for Implementing and Formally Verifying Distributed Systems,” PLDI 2015.  
https://doi.org/10.1145/2737924.2737958

### [S34] Aneris

Aneris project: higher-order separation logic for distributed systems.  
https://iris-project.org/aneris/

### [S35] Grove

Grove: distributed separation logic for realistic systems.  
https://github.com/mit-pdos/gokv  
https://doi.org/10.1145/3571202

### [S36] Perennial

Perennial: verifying concurrent crash-safe systems.  
https://github.com/mit-pdos/perennial

### [S37] Trillium

Trillium: higher-order concurrent and distributed separation logic/refinement.  
https://gitlab.mpi-sws.org/iris/trillium

### [S53] Strong observational refinement

Hagit Attiya and Constantin Enea, “Putting Strong Linearizability in Context,” DISC 2019; extended 2025 work.  
https://doi.org/10.4230/LIPIcs.DISC.2019.2  
https://doi.org/10.1007/s00236-025-00500-3

### [S54] Progressive forward simulation

Brijesh Dongol, Gerhard Schellhorn, Heike Wehrheim, “Weak Progressive Forward Simulation Is Necessary and Sufficient for Strong Observational Refinement,” CONCUR 2022.  
https://doi.org/10.4230/LIPIcs.CONCUR.2022.31

## H. Rust verification and semantics

### [S38] Verus

Verus official repository and guide.  
https://github.com/verus-lang/verus  
https://verus-lang.github.io/verus/guide/

### [S39] RustBelt

Ralf Jung et al., “RustBelt: Securing the Foundations of the Rust Programming Language,” POPL 2018.  
https://plv.mpi-sws.org/rustbelt/popl18/

### [S40] Creusot

Creusot official site.  
https://creusot.rs/

### [S41] Aeneas

Aeneas Rust-to-functional translation and verification project.  
https://github.com/AeneasVerif/aeneas

### [S42] Kani

Kani Rust verifier.  
https://model-checking.github.io/kani/  
https://github.com/model-checking/kani

### [S43] GenMC

GenMC stateless model checker.  
https://github.com/MPI-SWS/genmc

### [S44] Crux

Stuart Pernsteiner et al., “Crux, a Precise Verifier for Rust and Other Languages,” 2024.  
https://arxiv.org/abs/2410.18280  
https://crux.galois.com/

### [S45] hax/hacspec

hax high-assurance Rust translation.  
https://hax.cryspen.com/  
https://github.com/cryspen/hax  
https://hacspec.org/

### [S72] Library-defined capabilities for interior mutability

Federico Poli et al., “Reasoning about Interior Mutability in Rust using Library-Defined Capabilities,” 2024.  
https://arxiv.org/abs/2405.08372

### [S73] Gillian-Rust hybrid verification

Sacha-Élie Ayoun et al., “A Hybrid Approach to Semi-Automated Rust Verification,” 2024.  
https://arxiv.org/abs/2403.15122

## I. Proof certificates and trustworthy checking

### [S47] Alethe

Alethe proof format for SMT.  
https://verit.loria.fr/documentation/alethe-spec.pdf  
https://github.com/SMT-COMP/alethe

### [S48] Carcara

Carcara, Rust Alethe proof checker.  
https://github.com/ufmg-smite/carcara

### [S49] LRAT and verified checking

Nathan Wetzler, Marijn Heule, Warren Hunt, “DRAT-trim/LRAT” line of work and recent verified LRAT checkers.  
https://www.cs.utexas.edu/~marijn/publications/LRAT.pdf

### [S49A] CreuSAT

A SAT solver written in Rust and verified with Creusot.  
https://github.com/sarsko/CreuSAT

## J. Timed and probabilistic verification

### [S50] Storm

Storm probabilistic model checker.  
https://www.stormchecker.org/  
https://github.com/moves-rwth/storm

### [S51] UPPAAL

UPPAAL official site.  
https://uppaal.org/

### [S52] IMITATOR

IMITATOR parametric timed-automata tool.  
https://www.imitator.fr/

### [S56] PRISM

PRISM probabilistic model checker.  
https://www.prismmodelchecker.org/

### [S76] Timed multiparty sessions in Rust

Ping Hou, Nicolas Lagaillardie, Nobuko Yoshida, “Fearless Asynchronous Communications with Timed Multiparty Session Protocols,” ECOOP 2024.  
https://doi.org/10.4230/LIPIcs.ECOOP.2024.19

## K. Effects, sessions, choreographies, and synthesis

### [S57] Syntax-Guided Synthesis

Rajeev Alur et al., SyGuS.  
https://www.microsoft.com/en-us/research/publication/syntax-guided-synthesis/  
https://sygus.org/

### [S58] Formal theory of choreographic programming

Luís Cruz-Filipe, Fabrizio Montesi, Marco Peressotti, 2023.  
https://doi.org/10.1007/s10817-023-09665-3

### [S58A] Real-world choreographic programming

Lovro Lugović and Fabrizio Montesi, 2024.  
https://doi.org/10.22152/programming-journal.org/2024/8/8

### [S59] Refined multiparty protocols

Fangyi Zhou et al., “Statically Verified Refinements for Multiparty Protocols,” OOPSLA 2020.  
https://doi.org/10.1145/3428216

### [S59A] Hybrid multiparty session types

Lorenzo Gheri and Nobuko Yoshida, compositional protocol specification.  
https://arxiv.org/abs/2302.01979

### [S60] Algebraic effects and handlers

Andrej Bauer and Matija Pretnar, “An Effect System for Algebraic Effects and Handlers,” LMCS 2014.  
https://doi.org/10.2168/LMCS-10(4:9)2014

### [S60A] Higher-order effects

Birthe van den Berg and Tom Schrijvers, “A Framework for Higher-Order Effects & Handlers,” 2023.  
https://arxiv.org/abs/2302.01415

### [S60B] Relational separation logic for effect handlers

Paulo Emílio de Vilhena et al., POPL 2026.  
https://doi.org/10.1145/3776676

## L. Agent-assisted formal verification

### [S69] AutoVerus

Chenyuan Yang et al., “AutoVerus: Automated Proof Generation for Rust Code,” 2024.  
https://arxiv.org/abs/2409.13082

### [S70] VeriStruct

Chuyue Sun et al., “VeriStruct: AI-assisted Automated Verification of Data-Structure Modules in Verus,” 2025/2026.  
https://arxiv.org/abs/2510.25015

### [S70A] VeruSAGE

Chenyuan Yang et al., “VeruSAGE: A Study of Agent-Based Verification for Rust Systems,” preprint, 2025.  
https://arxiv.org/abs/2512.18436

## M. Architectural inspiration

### [S63] FrankenLean comprehensive plan

Jeff Emmanuel, `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKEN_LEAN.md`.  
https://github.com/Dicklesworthstone/franken_lean/blob/main/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKEN_LEAN.md

## Notes on evidence quality

- Official documentation and peer-reviewed papers are preferred.
- ArXiv entries from 2025–2026 are research signals, not settled facts.
- Tool capability claims must be rechecked against pinned versions before implementation decisions.
- Continuum's speculative research tracks do not become soundness claims merely because related mathematics exists.

## N. Additional 2025–2026 frontier work

### [S77] QSM-Cutoff

Yun-Rong Luo, Aman Goel, Karem Sakallah, “QSM-Cutoff: Systematic Derivation of Quantified Cutoff Formulas for Distributed Protocols,” CAV 2025.  
https://doi.org/10.1007/978-3-031-98682-6_14

### [S78] Await-aware optimal DPOR

Bengt Jonsson, Magnus Lång, Konstantinos Sagonas, “Awaiting for Godot: Stateless Model Checking that Avoids Executions Where Nothing Happens,” Formal Methods in System Design, 2025.  
https://doi.org/10.1007/s10703-025-00479-0

### [S79] Asynchronous runtime-verification decidability

Armando Castañeda, Gilde Valeria Rodríguez, “Asynchronous Fault-Tolerant Language Decidability for Runtime Verification of Distributed Systems,” preprint, 2025.  
https://arxiv.org/abs/2502.00191

### [S80] Verification algorithm selection

Roderick Bloem et al., “Btor2-Select: Machine Learning Based Algorithm Selection for Hardware Model Checking,” CAV 2025.  
https://doi.org/10.1007/978-3-031-98668-0_15

### [S81] RustMC

Oliver Pearce, Julien Lange, Dan O'Keeffe, “RustMC: Extending the GenMC Stateless Model Checker to Rust,” preprint/tool paper, 2025.  
https://arxiv.org/abs/2502.06293

### [S82] Partial-order conformance via unfoldings

Ariba Siddiqui, Wil M. P. van der Aalst, Daniel Schuster, “Computing Alignments for Partially-ordered Traces Through Petri Net Unfoldings,” preprint, 2025.  
https://arxiv.org/abs/2504.00550

Xixi Lu, Douwe Geurtjens, “FoldA: Computing Partial-Order Alignments Using Directed Net Unfoldings,” preprint, 2025.  
https://arxiv.org/abs/2506.08627

### [S83] Runtime verification over Mazurkiewicz traces

Martin Leucker, “A Note on Runtime Verification of Concurrent Systems,” 2025.  
https://www.isp.uni-luebeck.de/research/publications/note-runtime-verification-concurrent-systems

### [S84] LRAT-Catcher

Stefan Szeider, “LRAT-Catcher: Importing SAT Solver Certificates into Lean4 by Reflection,” preprint, 2026.  
https://arxiv.org/abs/2607.00815

### [S85] Three-dimensional refinement algebra

Yu Zhang, Jérémie Koenig, Yuting Wang, Zhong Shao, “Unifying Compositional Verification and Certified Compilation with a Three-Dimensional Refinement Algebra,” POPL 2025 artifact.  
https://doi.org/10.5281/zenodo.14065332  
https://github.com/CertiKOS/rbgs/tree/popl25-artifact

### [S86] Gillian-Rust, PLDI 2025

Sacha-Élie Ayoun et al., “A Hybrid Approach to Semi-Automated Rust Verification,” PLDI 2025.  
https://doi.org/10.1145/3729289  
https://gillianplatform.github.io/publications/rust.html

### [S87] Partial-order streaming conformance

“Back to the Order: Partial Orders in Streaming Conformance Checking,” Information Systems, 2025.  
https://doi.org/10.1016/j.is.2025.102566

### [S88] HyperLTL counterexamples and explanations

Sarah Winter, Martin Zimmermann, “Tracy, Traces, and Transducers: Computable Counterexamples and Explanations for HyperLTL Model-Checking,” Acta Informatica, 2025.  
https://doi.org/10.1007/s00236-025-00499-7

### [S89] Modular distributed hybrid analysis

Eduard Kamburjan, “Modular Analysis of Distributed Hybrid Systems Using Post-Regions,” Formal Methods in System Design, 2026.  
https://doi.org/10.1007/s10703-026-00491-y

## O. Combinatorial geometry of concurrency

### [S90] Median graphs, cube complexes, and event structures

Laurine Bénéteau, Jérémie Chalopin, Victor Chepoi, Yann Vaxès, “Medians in Median Graphs and Their Cube Complexes in Linear Time,” Journal of Computer and System Sciences, 2022.  
https://doi.org/10.1016/j.jcss.2022.01.001  
https://arxiv.org/abs/1907.10398

### [S91] Domains and event structures

Paolo Baldan, Andrea Corradini, Fabio Gadducci, “Domains and Event Structures for Fusions,” 2017.  
https://arxiv.org/abs/1701.02394

### [S92] Antimatroids in discrete-event systems

Paul Glasserman and David Yao, “Generalized Semi-Markov Processes: Antimatroid Structure and Second-Order Properties,” Mathematics of Operations Research 17(2), 1992.  
https://doi.org/10.1287/moor.17.2.444

### [S93] Trace-monoid combinatorics

Cartier–Foata trace-monoid theory and later work on dependence-graph growth and clique automata. One recent example:  
https://www.mdpi.com/2504-3900/123/1/8

## P. Revision-2 corpus, Lean, projection, and verification-frontier sources

### [S94] TLA+ Examples corpus

TLA+ Foundation, “TLA+ Examples,” pinned by Continuum revision 2 at commit `91c22ea537853196ed1e03e9ad91693ec37642de`. The repository serves as an example library, language-tool corpus, and case-study collection.  
https://github.com/tlaplus/Examples

### [S95] TLA+ Examples corpus tooling

Manifest generation, feature census, model-state recording, PlusCal translation, proof checking, and CI scripts.  
https://github.com/tlaplus/Examples/tree/master/.github/scripts

### [S96] TLAPS

TLA+ Proof Manager implementation and documentation.  
https://github.com/tlaplus/tlapm

### [S97] LeanLTL

Eric Vin, Kyle A. Miller, Daniel J. Fremont, “LeanLTL: A Unifying Framework for Linear Temporal Logics in Lean,” preprint, 2025.  
https://arxiv.org/abs/2507.01780

### [S98] PBLean

Stefan Szeider, “PBLean: Pseudo-Boolean Proof Certificates for Lean 4,” preprint, 2026.  
https://arxiv.org/abs/2602.08692

### [S99] Lean4Lean

Mario Carneiro, “Lean4Lean: Towards a Verified Typechecker for Lean, in Lean,” 2024.  
https://arxiv.org/abs/2403.14064

### [S100] Lean Kernel Arena

Independent Lean kernel/checker interoperability and benchmarking project.  
https://arena.lean-lang.org/

### [S101] Rust-to-Lean verification pipeline

Natalia Klaus, Palina Tolmach, Juan Conejero, “A Rust-to-Lean Verification Pipeline with AI Provers: An Experience Report,” preprint, 2026.  
https://arxiv.org/abs/2605.30106

### [S102] VeruSAGE

Chenyuan Yang et al., “VeruSAGE: A Study of Agent-Based Verification for Rust Systems,” preprint, 2025.  
https://arxiv.org/abs/2512.18436

### [S103] KVerus

Yuwei Liu et al., “KVerus: Scalable and Resilient Formal Verification Proof Generation for Rust Code,” preprint, 2026.  
https://arxiv.org/abs/2605.03822

### [S104] Lean-guided TLA+ proof automation

Yuhao Zhou and Stavros Tripakis, “Towards Language Model Guided TLA+ Proof Automation,” preprint, 2025.  
https://arxiv.org/abs/2512.09758

### [S105] Stateful partial-order reduction

Berk Cirisci et al., “A Pragmatic Approach to Stateful Partial Order Reduction,” 2022.  
https://arxiv.org/abs/2211.11942

### [S106] Data-centric DPOR

Marek Chalupa et al., “Data-Centric Dynamic Partial Order Reduction,” 2016.  
https://arxiv.org/abs/1610.01188

### [S107] DPOR for transaction isolation

Ahmed Bouajjani, Constantin Enea, Enrique Román-Calvo, “Dynamic Partial Order Reduction for Checking Correctness against Transaction Isolation Levels,” 2023.  
https://arxiv.org/abs/2303.12606

### [S108] Structural temporal logic

Eleftherios Ioannidis et al., “Structural Temporal Logic for Mechanized Program Verification,” 2024.  
https://arxiv.org/abs/2410.14906

### [S109] Sound and complete projection for global types

Dawit Tirore, Jesper Bengtson, Marco Carbone, “A Sound and Complete Projection for Global Types,” Journal of Automated Reasoning, 2025.  
https://doi.org/10.1007/s10817-025-09726-9

### [S110] Generalized asynchronous projection

Rupak Majumdar, Madhavan Mukund, Felix Stutz, Damien Zufferey, “Generalising Projection in Asynchronous Multiparty Session Types,” 2021.  
https://arxiv.org/abs/2107.03984

### [S111] Formal choreographic programming

Luís Cruz-Filipe, Fabrizio Montesi, Marco Peressotti, “A Formal Theory of Choreographic Programming,” 2022; mechanized in Coq.  
https://arxiv.org/abs/2209.01886

### [S112] Pirouette

Andrew K. Hirsch and Deepak Garg, “Pirouette: Higher-Order Typed Functional Choreographies,” POPL 2022; mechanized in Coq.  
https://doi.org/10.1145/3498684

### [S113] Multiparty asynchronous session types

Kohei Honda, Nobuko Yoshida, Marco Carbone, “Multiparty Asynchronous Session Types,” JACM 2016.  
https://doi.org/10.1145/2827695

### [S114] Event-structure semantics for multiparty sessions

“Event Structure Semantics for Multiparty Sessions,” Journal of Logical and Algebraic Methods in Programming, 2023.  
https://doi.org/10.1016/j.jlamp.2022.100844

### [S115] Weak-memory formalism survey

Roger C. Su and Robert J. Colvin, “Weak Memory Model Formalisms: Introduction and Survey,” 2026.  
https://doi.org/10.1002/cpe.70484

### [S116] Endive

William Schultz, Ian Dardik, Stavros Tripakis, “Plain and Simple Inductive Invariant Inference for Distributed Protocols in TLA+,” FMCAD 2022.  
https://arxiv.org/abs/2205.06360

### [S117] Scythe

Derek Egolf, William Schultz, Stavros Tripakis, “Efficient Synthesis of Symbolic Distributed Protocols by Sketching,” 2024.  
https://arxiv.org/abs/2405.07807

### [S118] Interpretation reduction

Derek Egolf and Stavros Tripakis, “Accelerating Protocol Synthesis and Detecting Unrealizability with Interpretation Reduction,” 2025.  
https://arxiv.org/abs/2501.14585

### [S119] IC3PO

Aman Goel and Karem A. Sakallah, “On Symmetry and Quantification: A New Approach to Verify Distributed Protocols,” 2021.  
https://arxiv.org/abs/2103.14831

### [S120] Inductive proof slicing

William Schultz, Edward Ashton, Heidi Howard, Stavros Tripakis, “Scalable, Interpretable Distributed Protocol Verification by Inductive Proof Slicing,” 2024.  
https://arxiv.org/abs/2404.18048

### [S121] Shipwright

Derek Leung, Nickolai Zeldovich, Frans Kaashoek, “Shipwright: Proving Liveness of Distributed Systems with Byzantine Participants,” preprint, 2025.  
https://arxiv.org/abs/2507.14080

### [S122] Regular abstractions for array systems

Chih-Duo Hong and Anthony W. Lin, “Regular Abstractions for Array Systems,” POPL 2024.  
https://arxiv.org/abs/2401.02618

### [S123] Automated cutoff-based verification

Shreesha G. Bhat and Kartik Nagar, “Automating and Mechanizing Cutoff-based Verification of Distributed Protocols,” 2022/2023.  
https://arxiv.org/abs/2211.15175

### [S124] Ranking-function liveness verification

Jianan Yao, Runzhou Tao, Ronghui Gu, Jason Nieh, “Mostly Automated Verification of Liveness Properties for Distributed Protocols with Ranking Functions,” POPL 2024.  
https://doi.org/10.1145/3632877

### [S125] IC3Syn

Weining Cao et al., “Synthesizing Inductive Invariants for Distributed Protocols via IC3 and Large Language Models,” preprint, 2026. Candidate invariants are independently checked; the preprint reports TLAPS proofs for unbounded instances.  
https://arxiv.org/abs/2605.24619

## M. Agent-computer interfaces, proof agents, and developer experience

### [S126] SWE-agent and Agent–Computer Interfaces

John Yang et al., “SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering,” NeurIPS 2024. The work isolates interface design as a major determinant of coding-agent effectiveness.  
https://arxiv.org/abs/2405.15793

### [S127] Pantograph

Leni Aniva et al., “Pantograph: A Machine-to-Machine Interaction Interface for Advanced Theorem Proving, High Level Reasoning, and Data Extraction in Lean 4,” TACAS 2025.  
https://arxiv.org/abs/2410.16429  
https://doi.org/10.1007/978-3-031-90643-5_6

### [S128] AXLE

Jimmy Xin et al., “AXLE: A Cloud Infrastructure for Lean 4 Theorem Proving Utilities,” preprint, 2026.  
https://arxiv.org/abs/2606.26442

### [S129] OProver

David Ma et al., “OProver: A Unified Framework for Agentic Formal Theorem Proving,” preprint, 2026.  
https://arxiv.org/abs/2605.17283

### [S130] LAMP

Santhana Srinivasan R and Maithilee Patawar, “LAMP: Lean-based Agentic framework with MCP and Proof Repair,” preprint, 2026.  
https://arxiv.org/abs/2606.28841

### [S131] LeanDojo

Kaiyu Yang et al., “LeanDojo: Theorem Proving with Retrieval-Augmented Language Models,” NeurIPS 2023.  
https://arxiv.org/abs/2306.15626  
https://github.com/lean-dojo/LeanDojo

### [S132] SWE-Effi

Zhiyu Fan et al., “SWE-Effi: Re-Evaluating Software AI Agent System Effectiveness Under Resource Constraints,” preprint, 2025.  
https://arxiv.org/abs/2509.09853

## N. Interaction protocols and report formats

### [S133] Language Server Protocol 3.18

Microsoft, official Language Server Protocol specification.  
https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/

### [S134] Debug Adapter Protocol 1.71

Microsoft, official Debug Adapter Protocol specification.  
https://microsoft.github.io/debug-adapter-protocol/specification

### [S135] SARIF 2.1.0

OASIS Static Analysis Results Interchange Format specification.  
https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html

### [S136] MCP explicit state handles

Model Context Protocol SEP-2567, “Sessionless MCP via Explicit State Handles,” Final Standards Track, 2026.  
https://modelcontextprotocol.io/seps/2567-sessionless-mcp

## O. Counterexample explanation and human factors

### [S137] Counterexample explanation systematic review

Arut Prakash Kaleeswaran et al., “A systematic literature review on counterexample explanation,” 2022.  
https://arxiv.org/abs/2201.03061

### [S138] Bosch formal-verification explanation study

Arut Prakash Kaleeswaran et al., “A User Study for Evaluation of Formal Verification Results and their Explanation at Bosch,” Empirical Software Engineering, 2023.  
https://arxiv.org/abs/2304.08950  
https://doi.org/10.1007/s10664-023-10353-4

### [S139] Halpern–Pearl actual causality

Joseph Y. Halpern and Judea Pearl, foundational structural-model definitions of actual causality and subsequent refinements.  
https://www.cs.cornell.edu/home/halpern/papers/causalitybook-ch1-3.html

## P. Incrementality and bidirectional correspondence

### [S140] Salsa

Salsa, incremental computation framework for Rust and design documentation.  
https://salsa-rs.github.io/salsa/  
https://github.com/salsa-rs/salsa

### [S141] Certifying incremental SAT solving

Katalin Fazekas, Florian Pollitt, Mathias Fleury, Armin Biere, “Certifying Incremental SAT Solving,” LPAR 2024.  
https://doi.org/10.29007/pdcc  
https://easychair.org/publications/paper/TbPs

### [S142] Unrealizability proof synthesis and reusable summaries

Shaan Nagy et al., “Automating Unrealizability Logic: Hoare-Style Proof Synthesis for Infinite Sets of Programs,” 2024.  
https://arxiv.org/abs/2401.13244

### [S143] KBX

Jianhong Zhao et al., “KBX: Verified Model Synchronization via Formal Bidirectional Transformation,” 2024.  
https://arxiv.org/abs/2404.18771

### [S144] Effectful lenses

Ruifeng Xie, Tom Schrijvers, Zhenjiang Hu, “Effectful Lenses: There and Back with Different Monads,” ICFP 2025; formalized in Agda.  
https://doi.org/10.1145/3747523

### [S145] BiGUL

Hsiang-Shang Ko, Tao Zan, Zhenjiang Hu, “BiGUL: a formally verified core language for putback-based bidirectional programming,” PEPM 2016.  
https://doi.org/10.1145/2847538.2847544

## Q. Synthesis, diversity, and verified invention

### [S146] SyGuS

Syntax-Guided Synthesis community, standard and benchmark resources.  
https://sygus-org.github.io/

### [S147] Quality-diversity algorithms

Jean-Baptiste Mouret and Jeff Clune, “Illuminating search spaces by mapping elites,” 2015, and subsequent MAP-Elites/quality-diversity literature.  
https://arxiv.org/abs/1504.04909

### [S148] Diversifying to Verify

“Diversifying to Verify: When Task-Equivalent Programs Differ in Verifiability,” preprint, 2026. The paper studies how semantically equivalent programs can differ sharply in downstream formal-verification success, motivating proof-aware quality-diversity search.  
https://arxiv.org/abs/2607.09366

### [S149] Vericoding benchmark

“Vericoding: Benchmarking Code Generation with Formal Verification,” preprint, 2025.  
https://arxiv.org/abs/2509.22908

### [S150] SEVerA

“SEVerA: Specification-Evolving Verification Agent,” preprint, 2026. A contemporary baseline for agents that co-evolve implementation, specification, and proof obligations.  
https://arxiv.org/abs/2603.25111

### [S151] MOUR-QD

Research on uncertainty-aware quality-diversity optimization for robust exploration, used as a comparative baseline for Forge's repertoire search.  
https://doi.org/10.1145/3712256.3726394
