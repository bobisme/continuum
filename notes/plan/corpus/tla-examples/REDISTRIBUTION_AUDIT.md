# Corpus Redistribution Audit

**Scope:** every family in the pinned corpus inventory — the 80 CI-validated
specification families in [`validated-examples.csv`](validated-examples.csv) and the 39
tracked additional examples in [`other-examples.csv`](other-examples.csv).
**Pinned upstream commit:** `91c22ea537853196ed1e03e9ad91693ec37642de`
**Audited:** 2026-07-31
**Required by:** plan §21.1 (per-family redistribution audit before any public benchmark
release), PR 0 (`notes/START_HERE_IMPLEMENTATION.md`), [ADR-0021](../../adr/0021-tla-examples-corpus-contract.md),
[`CORPUS_POLICY.md`](CORPUS_POLICY.md) ("Source provenance and licensing"), and
[docs/45](../../docs/45_CONTINUUMBENCH.md) ("TLA+ Examples corpus, with license/provenance preserved").

This is a factual audit prepared by an implementation agent. It records what was verified
and what was not. It is not legal advice, and it does not authorize a release: the
publication decision remains privileged.

## 1. Method

1. The upstream tree at the pinned commit was enumerated in full (1,177 entries, GitHub
   reports `truncated: false`); every path was checked for `LICENSE`, `COPYING`, or
   `NOTICE` files.
2. Third-party repository licenses were read from GitHub's license endpoint. Every
   `NOASSERTION` result was resolved by reading the license file text itself rather than
   trusting the classifier.
3. Directory contents were counted per family to distinguish real in-tree specifications
   from catalog stubs.

Reproduction:

```sh
gh api "repos/tlaplus/Examples/git/trees/91c22ea537853196ed1e03e9ad91693ec37642de?recursive=1" \
  --jq '.tree[].path' | grep -iE 'licen|copying|notice'
gh api repos/<owner>/<repo> --jq 'if .license == null then "NO-LICENSE-FILE" else .license.spdx_id end'
```

Nothing below is inferred from a specification's content, an author's affiliation, or a
project's reputation. An entry whose grant could not be verified is recorded as `unclear`
or `blocked`, never as "probably permissive".

## 2. Verdict vocabulary

- **cleared** — a permissive grant was verified at the source; redistribution in a public
  benchmark is permitted with the copyright notice and license text preserved.
- **unclear** — content exists but the governing grant was not established by this audit
  (site-hosted bundle not inspected, or conflicting license metadata). Requires an
  upstream check before redistribution.
- **blocked** — no grant was found (verified absence of a license file, i.e. default
  all-rights-reserved) or the artifact carries third-party publisher copyright.
  Redistribution is not permitted; link to it instead.

`cleared` never means "no obligations". Every cleared row still requires notice
preservation, and the BSD/Apache rows carry their own additional conditions.

## 3. Repository-level grant

`LICENSE.md` at the pinned commit is the MIT License, copyright "The TLA+ project and
other contributors", 2016.

The upstream README qualifies it: *"The repository is under the MIT license and you are
encouraged to publish your spec under a similarly-permissive license. However, your spec
can be included in this repo along with your own license if you wish."*

The repository grant is therefore a default, not a blanket. A tree-wide scan found exactly
three per-family license files (all permissive) plus one third-party library notice:

| Path | License | Affects |
|---|---|---|
| `specifications/KeyValueStore/LICENSE` | BSD-2-Clause (ING Bank / CWI, 2020) | TV-038 |
| `specifications/SDP_Verification/LICENSE` | MIT (Dong Luming, 2022) | TV-052 |
| `specifications/byihive/LICENSE` | Apache-2.0 | TV-071 |
| `specifications/ewd998/impl/lib/gson-LICENSE` | Apache-2.0 (bundled `gson-2.8.6.jar`) | TV-025 |

## 4. Validated families (80) — all cleared

Every one of the 80 validated families has real in-tree content at the pinned commit and
is covered by a verified permissive grant: 77 by the repository MIT license and 3 by their
own permissive license.

| ID | Family | Upstream path (pinned commit) | Governing license | Redistribution | Notes |
|---|---|---|---|---|---|
| TV-001 | Teaching Concurrency | `specifications/TeachingConcurrency` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-002 | Loop Invariance | `specifications/LoopInvariance` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-003 | Learn TLA+ Proofs | `specifications/LearnProofs` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-004 | Boyer-Moore Majority Vote | `specifications/Majority` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-005 | Proof x+x is Even | `specifications/sums_even` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-006 | The N-Queens Puzzle | `specifications/N-Queens` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-007 | The Dining Philosophers Problem | `specifications/DiningPhilosophers` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-008 | The Car Talk Puzzle | `specifications/CarTalkPuzzle` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-009 | The Die Hard Problem | `specifications/DieHard` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-010 | The Prisoners & Switches Puzzle | `specifications/Prisoners` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-011 | The Prisoners & Switch Puzzle (single switch) | `specifications/Prisoners_Single_Switch` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-012 | Specs from Specifying Systems | `specifications/SpecifyingSystems` | MIT (repository `LICENSE.md`) | cleared | Modules originate from Lamport's *Specifying Systems*; upstream ships them under the repository grant with no separate notice. Courtesy confirmation with the TLA+ Foundation is advisable before a commercial benchmark release. |
| TV-013 | The Tower of Hanoi Puzzle | `specifications/tower_of_hanoi` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-014 | Missionaries and Cannibals | `specifications/MissionariesAndCannibals` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-015 | Stone Scale Puzzle | `specifications/Stones` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-016 | The Coffee Can Bean Problem | `specifications/CoffeeCan` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-017 | EWD687a: Detecting Termination in Distributed Computations | `specifications/ewd687a` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-018 | The Moving Cat Puzzle | `specifications/Moving_Cat_Puzzle` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-019 | The Boulangerie Algorithm | `specifications/Bakery-Boulangerie` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-020 | Misra Reachability Algorithm | `specifications/MisraReachability` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-021 | Byzantizing Paxos by Refinement | `specifications/byzpaxos` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-022 | Barrier Synchronization | `specifications/barriers` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-023 | Peterson Lock Refinement With Auxiliary Variables | `specifications/locks_auxiliary_vars` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-024 | EWD840: Termination Detection in a Ring | `specifications/ewd840` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-025 | EWD998: Termination Detection with Asynchronous Delivery | `specifications/ewd998` | MIT (repository `LICENSE.md`) | cleared | The family tree vendors `impl/lib/gson-2.8.6.jar` plus `impl/lib/gson-LICENSE` (third-party Java binary). Exclude `impl/` from any redistributed subset or carry the gson notice; it is also exactly the class of Java artifact ADR-0029 keeps out of release binaries. |
| TV-026 | The Paxos Protocol | `specifications/Paxos` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-027 | Asynchronous Reliable Broadcast | `specifications/bcastByz` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-028 | Distributed Mutual Exclusion | `specifications/lamport_mutex` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-029 | Two-Phase Handshaking | `specifications/TwoPhase` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-030 | Paxos (How to Win a Turing Award) | `specifications/PaxosHowToWinATuringAward` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-031 | Finitizing Monotonic Systems | `specifications/FiniteMonotonic` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-032 | Dijkstra's Mutual Exclusion Algorithm | `specifications/dijkstra-mutex` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-033 | The Echo Algorithm | `specifications/echo` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-034 | The TLC Safety Checking Algorithm | `specifications/TLC` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-035 | Transaction Commit Models | `specifications/transaction_commit` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-036 | The Slush Protocol | `specifications/SlushProtocol` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-037 | Minimal Circular Substring | `specifications/LeastCircularSubstring` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-038 | Snapshot Key-Value Store | `specifications/KeyValueStore` | BSD-2-Clause (ING Bank / CWI, 2020) | cleared | `specifications/KeyValueStore/LICENSE`, scoped to `ClientCentric.tla` |
| TV-039 | Chang-Roberts Leader Election | `specifications/chang_roberts` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-040 | MultiPaxos in SMR-Style | `specifications/MultiPaxos-SMR` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-041 | Einstein's Riddle | `specifications/EinsteinRiddle` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-042 | Resource Allocator | `specifications/allocator` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-043 | Transitive Closure | `specifications/TransitiveClosure` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-044 | Atomic Commitment Protocol | `specifications/acp` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-045 | SWMR Shared Memory Disk Paxos | `specifications/diskpaxos` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-046 | Span Tree Exercise | `specifications/SpanningTree` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-047 | The Knuth-Yao Method | `specifications/KnuthYao` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-048 | Huang's Algorithm | `specifications/Huang` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-049 | EWD 426: Token Stabilization | `specifications/ewd426` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-050 | Sliding Block Puzzle | `specifications/SlidingPuzzles` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-051 | Single-Lane Bridge Problem | `specifications/SingleLaneBridge` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-052 | Software-Defined Perimeter | `specifications/SDP_Verification` | MIT (Dong Luming, 2022) | cleared | `specifications/SDP_Verification/LICENSE` |
| TV-053 | Simplified Fast Paxos | `specifications/SimplifiedFastPaxos` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-054 | Checkpoint Coordination | `specifications/CheckpointCoordination` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-055 | Multi-Car Elevator System | `specifications/MultiCarElevator` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-056 | Nano Blockchain Protocol | `specifications/NanoBlockchain` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-057 | The Readers-Writers Problem | `specifications/ReadersWriters` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-058 | Asynchronous Byzantine Consensus | `specifications/aba-asyn-byz` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-059 | Folklore Reliable Broadcast | `specifications/bcastFolklore` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-060 | The Bosco Byzantine Consensus Algorithm | `specifications/bosco` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-061 | Consensus in One Communication Step | `specifications/c1cs` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-062 | One-Step Consensus with Zero-Degradation | `specifications/cf1s-folklore` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-063 | Failure Detector | `specifications/detector_chan96` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-064 | Asynchronous Non-Blocking Atomic Commit | `specifications/nbacc_ray97` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-065 | NBAC with Failure Detectors | `specifications/nbacg_guer01` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-066 | Spanning Tree Broadcast Algorithm | `specifications/spanning` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-067 | The Cigarette Smokers Problem | `specifications/CigaretteSmokers` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-068 | Conway's Game of Life | `specifications/GameOfLife` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-069 | Chameneos, a Concurrency Game | `specifications/Chameneos` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-070 | PCR Testing for Snippets of DNA | `specifications/glowingRaccoon` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-071 | RFC 3506: Voucher Transaction System | `specifications/byihive` | Apache-2.0 | cleared | `specifications/byihive/LICENSE` |
| TV-072 | Yo-Yo Leader Election | `specifications/YoYo` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-073 | TCP as defined in RFC 9293 | `specifications/tcp` | MIT (repository `LICENSE.md`) | cleared | Model of RFC 9293. The TLA+ text is original in-tree work under the repository grant; the RFC itself (IETF Trust) is referenced, not redistributed. |
| TV-074 | B-trees | `specifications/btree` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-075 | TLA+ Level Checking | `specifications/LevelChecking` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-076 | Condition-Based Consensus | `specifications/cbc_max` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-077 | Buffered Random Access File | `specifications/braf` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-078 | Disruptor | `specifications/Disruptor` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-079 | DAG-based Consensus | `specifications/dag-consensus` | MIT (repository `LICENSE.md`) | cleared | — |
| TV-080 | German Cache-Coherence Protocol | `specifications/GermanProtocol` | MIT (repository `LICENSE.md`) | cleared | — |

### 4.1 Watch items inside the cleared set

- **TV-025 (EWD998)** vendors a third-party Java binary (`impl/lib/gson-2.8.6.jar`). A
  redistributed corpus subset should exclude `impl/` or carry the gson Apache-2.0 notice.
  The same artifact is a live example of what [ADR-0029](../../adr/0029-semantic-equivalence-not-tla-source-cloning.md)
  keeps out of release binaries.
- **TV-038 (Snapshot Key-Value Store)** is BSD-2-Clause and its notice is scoped to
  `ClientCentric.tla`; the binary-form clause applies if the corpus ships in a packaged
  artifact.
- **TV-012 (Specs from Specifying Systems)** consists of modules originating in Lamport's
  *Specifying Systems*. Upstream ships them under the repository grant with no separate
  notice, which is what this audit records. A courtesy confirmation with the TLA+
  Foundation is advisable before a commercial benchmark release.
- **TV-071 (byihive)** is Apache-2.0, which adds NOTICE-file and modification-marking
  obligations the MIT rows do not have.

## 5. Additional tracked examples (39)

The inventory's location classes describe the upstream catalog, not the presence of
redistributable content. At the pinned commit, **every "in-tree" additional example is a
catalog stub**: each of the 25 corresponding directories contains exactly one file, a
`README.md` naming the specification's author, the original paper, and a link to the
third-party source. There is no TLA+ source for any of these families in the pinned
corpus, so there is nothing to redistribute from upstream; a port must fetch from the
third-party source recorded below and inherit that source's license.

| ID | Example | Inventory class | Actual source | License | Redistribution | Evidence |
|---|---|---|---|---|---|---|
| TO-001 | Blocking Queue | external | https://github.com/lemmy/BlockingQueue (upstream submodule `specifications/BlockingQueue`) | MIT | cleared | GitHub license endpoint |
| TO-002 | IEEE 802.16 WiMAX Protocols | external | http://list.cs.northwestern.edu/802.16/ | not established | unclear | University-hosted page; not inspected in this audit, no grant observed upstream |
| TO-003 | On the diversity of asynchronous communication | external | http://hurault.perso.enseeiht.fr/asynchronousCommunication/ (stub `specifications/async-comm`) | not established | unclear | Personal academic site; not inspected in this audit |
| TO-004 | Caesar | in-tree-unvalidated | https://github.com/nano-o/Caesar | MIT | cleared | GitHub license endpoint |
| TO-005 | CASPaxos | in-tree-unvalidated | https://github.com/tschottdorf/caspaxos-tla | none found | blocked | GitHub license endpoint returns no license: default all-rights-reserved |
| TO-006 | DataPort | in-tree-pdf | IEEE Xplore PDF (`ieeexplore.ieee.org/iel7/7858577/7862346/07862411.pdf`) | publisher copyright | blocked | Publisher-hosted paper; no redistribution grant |
| TO-007 | egalitarian-paxos | in-tree-unvalidated | https://github.com/efficient/epaxos | Apache-2.0 (Carnegie Mellon University, 2013) | cleared | License file read directly; GitHub reports NOASSERTION |
| TO-008 | fastpaxos | in-tree-pdf | Microsoft Research publication page (Fast Paxos) | publisher terms | blocked | Publisher-hosted paper; no redistribution grant |
| TO-009 | fpaxos | in-tree-unvalidated | https://github.com/fpaxos/fpaxos-tlaplus | MIT (Heidi Howard, 2016) | cleared | License file read directly; GitHub reports NOASSERTION |
| TO-010 | HLC | in-tree-unvalidated | https://github.com/muratdem/HLC | none found | blocked | GitHub license endpoint returns no license |
| TO-011 | L1 | in-tree-pdf | Microsoft Research publication page (L1 switch protocol) | publisher terms | blocked | Publisher-hosted paper; no redistribution grant |
| TO-012 | leaderless | in-tree-unvalidated | https://losa.fr/research/leaderless/ | not established | unclear | Personal academic site; download bundle not inspected |
| TO-013 | losa_ap | in-tree-unvalidated | https://losa.fr/research/assignment/ | not established | unclear | Personal academic site; download bundle not inspected |
| TO-014 | losa_rda | in-tree-pdf | https://www.losa.fr/Thesis.pdf | thesis copyright | blocked | PDF only; no redistribution grant |
| TO-015 | m2paxos | in-tree-unvalidated | https://losa.fr/M2Paxos/ | not established | unclear | Personal academic site; download bundle not inspected |
| TO-016 | mongo-repl-tla | in-tree-unvalidated | https://github.com/visualzhou/mongo-repl-tla | none found | blocked | GitHub license endpoint returns no license |
| TO-017 | MultiPaxos | in-tree-unvalidated | https://github.com/nano-o/MultiPaxos | MIT | cleared | GitHub license endpoint |
| TO-018 | naiad | in-tree-pdf | Microsoft Research PDF (Naiad) | publisher terms | blocked | PDF only; no redistribution grant |
| TO-019 | nfc04 | in-tree-unvalidated | http://www.steffen-zschaler.de/publications/NfC04/ | not established | unclear | Personal academic site; not inspected in this audit |
| TO-020 | raft | in-tree-unvalidated | https://github.com/ongardie/raft.tla | none found | blocked | GitHub license endpoint returns no license |
| TO-021 | SnapshotIsolation | in-tree-unvalidated | https://github.com/sanjosh/tlaplus (`amazon/serializableSnapshotIsolation.tla`) | Apache-2.0 reported | unclear | Endpoint reports Apache-2.0 for a fork whose upstream `tlaplus/tlaplus` is MIT; the discrepancy needs resolution before redistribution |
| TO-022 | SyncConsensus | in-tree-unvalidated | https://github.com/muratdem/PlusCal-examples | none found | blocked | GitHub license endpoint returns no license |
| TO-023 | Termination | in-tree-unvalidated | https://github.com/nano-o/Distributed-termination-detection | MIT | cleared | GitHub license endpoint |
| TO-024 | Tla-tortoise-hare | in-tree-unvalidated | https://github.com/lorin/tla-tortoise-hare | MIT | cleared | GitHub license endpoint |
| TO-025 | VoldemortKV | in-tree-unvalidated | https://github.com/muratdem/PlusCal-examples | none found | blocked | GitHub license endpoint returns no license |
| TO-026 | Tencent-Paxos | in-tree-unvalidated | https://github.com/Starydark/Tencent-Paxos-TLA (stub `specifications/TencentPaxos`) | none found | blocked | GitHub license endpoint returns no license |
| TO-027 | Paxos (external) | external | https://github.com/neoschizomer/Paxos | MIT | cleared | GitHub license endpoint |
| TO-028 | Lock-Free Set | external | `tlaplus/tlaplus` `OpenAddressing.tla` | MIT | cleared | GitHub license endpoint |
| TO-029 | ParallelRaft | in-tree-unvalidated | https://github.com/HappyCS-Gu/Parallel-Raft-tla (stub `specifications/ParalleRaft`) | MIT | cleared | GitHub license endpoint |
| TO-030 | CRDT-Bug | external | https://github.com/Alexander-N/tla-specs (`crdt-bug`) | MIT | cleared | GitHub license endpoint |
| TO-031 | asyncio-lock | external | https://github.com/Alexander-N/tla-specs (`asyncio-lock`) | MIT | cleared | GitHub license endpoint |
| TO-032 | Raft with cluster changes | external | https://github.com/dranov/raft-tla | CC-BY-4.0 | cleared | License file read directly; GitHub reports NOASSERTION. Attribution required; CC-BY is a content license, not a software license |
| TO-033 | MET for CRDT-Redis | external | https://github.com/elem-azar-unis/CRDT-Redis (`MET/TLA`) | BSD-3-Clause | cleared | GitHub license endpoint |
| TO-034 | Parallel increment | external | https://github.com/Cjen1/tla_increment | none found | blocked | GitHub license endpoint returns no license |
| TO-035 | Streamlet | external | https://github.com/nano-o/streamlet | MIT | cleared | GitHub license endpoint |
| TO-036 | Petri Nets | external | https://github.com/elh/petri-tlaplus | MIT | cleared | GitHub license endpoint |
| TO-037 | CRDT | external | https://github.com/JYwellin/CRDT-TLA | none found | blocked | GitHub license endpoint returns no license |
| TO-038 | Azure Cosmos DB | external | https://github.com/tlaplus/azure-cosmos-tla (upstream submodule) | MIT | cleared | GitHub license endpoint |
| TO-039 | Simple Microwave Oven | external | https://github.com/lucformalmethodscourse/microwave-tla (upstream submodule) | MIT | cleared | GitHub license endpoint |

**Counts: 18 cleared, 7 unclear, 14 blocked.**

## 6. Upstream submodules (4)

The upstream repository references four submodules. A submodule is a pointer, not content:
none of these is covered by the `tlaplus/Examples` MIT grant, and each carries its own
license.

| Submodule path | Repository | License | Redistribution |
|---|---|---|---|
| `specifications/CCF` | `microsoft/CCF` | Apache-2.0 | cleared |
| `specifications/azure-cosmos-tla` | `tlaplus/azure-cosmos-tla` | MIT | cleared (TO-038) |
| `specifications/BlockingQueue` | `lemmy/BlockingQueue` | MIT | cleared (TO-001) |
| `specifications/microwave` | `lucformalmethodscourse/microwave-tla` | MIT | cleared (TO-039) |

**Inventory gap:** `specifications/CCF` has no row in either inventory CSV. It is upstream
content at the pinned commit that the dossier's corpus inventory does not track. This
audit records the gap; whether CCF becomes a tracked family is a corpus-scope decision,
not an audit finding.

## 7. Consequences for a public benchmark release

1. The mandatory corpus — all 80 validated families — is redistributable. The ADR-0021
   release contract is not blocked by licensing.
2. A benchmark artifact that redistributes corpus source must carry: the upstream MIT
   notice, the three per-family notices from §3, and the gson notice if `impl/` is
   included.
3. Of the 39 additional examples, 18 may be redistributed with attribution, 7 need an
   upstream check first, and 14 may only be linked. Under
   [`CORPUS_POLICY.md`](CORPUS_POLICY.md) ("External examples remain links plus
   independently authored native models when necessary"), the 14 blocked entries are
   handled exactly as the policy already prescribes — this audit changes nothing about how
   they are ported, only about what may be shipped.
4. Continuum's native ports are independently authored artifacts. Their redistribution is
   governed by the product license decision, not by this audit; the audit governs copies
   of upstream source and any generated artifact that embeds it.

## 8. Open items requiring an upstream check

| Item | Action |
|---|---|
| 7 `unclear` rows (TO-002, TO-003, TO-012, TO-013, TO-015, TO-019, TO-021) | Inspect the published bundle for a license file; where absent, ask the author for an explicit grant. TO-021's fork/upstream license discrepancy must be resolved specifically. |
| 14 `blocked` rows | Either request a license grant from the author (most are single-author repositories where an issue asking for a `LICENSE` file is the whole remedy) or ship links only. |
| TV-012 provenance | Courtesy confirmation with the TLA+ Foundation on the *Specifying Systems* modules. |
| `specifications/CCF` | Decide whether to track it in the inventory; it is Apache-2.0 either way. |
| Corpus epoch bumps | Re-run this audit on every pin change. Licenses are a function of the pinned commit, and a new epoch may add a family with its own terms. |

## 9. What this audit does not establish

- It does not inspect every file header inside the 80 cleared families for an
  inconsistent per-file copyright notice; it establishes the governing grant at the
  repository and family-directory level.
- It does not evaluate patent exposure, trademark use of protocol names, or the
  publication rights of the papers the specifications model.
- It does not decide anything. Publication of a public benchmark remains a privileged
  decision.
