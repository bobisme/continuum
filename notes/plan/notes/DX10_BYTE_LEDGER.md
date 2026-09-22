# DX-10 byte ledger — decomposition, redesign candidates, and the sum-check

**Prepared:** 2026-08-09 (bone `bn-3bn62`, workspace epoch `b93fb14`)
**Commissioned by:** the X1 adjudication on `bn-762i` (2026-08-09) — "X1 = agent-visible
accounting, the landed reading … G0-DX-10 FAILS AS MEASURED and protocol redesign before
freeze is FORCED", item (2): "byte-ledger decomposition of the artifacts 20,732 B cost
naming redesign targets".
**Instrument:** `crates/continuum-benchmark`, the `pr10_*` and `dx10_falsification` tests
and the typed report artifacts they render.
**Scope:** analysis and planning. This note changes no harness code, no RFC, and no matrix
row. It produces measured tables, one redesign candidate per ledger item, a sum-check
against the ratified margin, and an implementation order.

---

## 0. What this note concludes, up front

1. The `artifacts` 20,732 B is **not by-value payload**. Every member of an `ArtifactRef` is
   already a reference. The cost is **re-transmission of references the connection already
   carried** (63.9% of refs name an artifact the same run already named) plus one member
   (`kind`, 2,040 B) whose value is a syntactic substring of the member beside it.
2. Four of the five ledger items are **connection constants charged per answer**: the epoch
   set is one distinct value across all 172 answers, the assurance envelope is one distinct
   value across all 24 answers that carry it, the omission manifest is four distinct shapes
   each a function of `(operation, admitted)`, and every one of the 324 omission records
   carries `reason = "unsupported"` — a statement about the deployment, not about the answer.
3. The decomposition found **two costs the ledger did not name** and which are larger than
   three of the five it did: 31,232 B of re-sent content digests (23.6% of all result bytes
   are 64-hex identities; 77.7% of those bytes are repeats) and 34,800 B of `task.status`
   record re-transmission (69% of all payload bytes).
4. **The sum-check fails.** The five commissioned items, fully realised, move the bytes
   margin from **−163% to −81%**. Adding both discoveries reaches **−38%**. The ratified
   +30% floor needs the native arm at **≤ 2,882 B per solved task**; the best candidate set
   measured here lands at **5,685**. §5 gives the algebra that says why no redesign of this
   encoding can reach it, and what could.
5. That is not a reason to abandon the redesign. The exit sentence is a **disjunction** —
   "measurably more effective **or** the protocol is redesigned before freeze" — and the
   ruling already took the second branch. The candidate set removes **47.6% of the native
   arm's total wire spend, drops no protocol guarantee, and costs zero extra round trips on
   all 24 solved tasks of the instrument's mix.** §4 states this as the honest deliverable
   and §5 states what the lead must decide before anyone implements toward a passing row.
6. **Erratum, 2026-08-09 — item C7 is falsified by measurement.** `bn-6fuu5` re-ran C7's
   projection against the mechanism the declaration actually admits, and found the saving
   negative and the wire-visibility class wrong. Points 4 and 5 stay above as the record of
   what was projected. Corrected: the candidate set is the **six-item 3.6 set**, worth
   **99,780 B over the matrix = 4,060 B per solved task**, which lands the native arm at
   **6,799 B per solved task** and a **−65%** margin, and removes **37.4%** of the native
   arm's total wire spend, not 47.6%. Both conclusions hold and both get stronger: the
   sum-check fails by more, and the set now costs no extra round trip for any client rather
   than none on this mix alone. §3 C7 carries the falsification, §4 the corrected sum-check,
   §6 the corrected order.
7. **Erratum, 2026-08-09 — the wire-visibility column is falsified for the whole set, and the
   3.6 bundle is near-empty.** `bn-2in7i` falsified C5's class the same way `bn-6fuu5`
   falsified C7's, and commissioned a presence-marker pre-check of the five that were never
   checked. `bn-d2bdi` ran it: **C1, C2, C3, C4a and C6 are major-gated**, and the only
   survivor is **C4b in a modified form — a new operation, not the declared trim — worth 494 B
   per solved task**, 6.2% of the gap the ratified floor needs. Of the corrected six-item 3.6
   set's 4,060 B per solved task, **3,566 B (87.8%) moves to the 4.0 horizon**. §8 carries the
   verdicts, the quoted declarations, the round-2 errata and the restated sum-check. Both
   conclusions of points 4 and 5 hold unchanged; what changes is the *version* at which any of
   it is available, and that is a decision above this note.

---

## 1. Method, and how to reproduce every number here

Every figure below is measured, not modelled. The decomposition runs the baseline sweep in
recording mode (`continuum_benchmark::variants::shell_sweep(Renderer::Standard,
Disciplines::ALL, true)`), which retains every `ResultEnvelope` the CLI process received;
each envelope is re-encoded with `continuumd`'s own canonical-JSON codec with one field at a
time reduced, exactly as `variants::decompose` does. The candidate projections re-encode the
same envelopes with the candidate's transform applied, in run order, with a connection-scoped
"already seen" set — so an amortization claim is measured against the frames that actually
preceded it rather than assumed.

The landed anchors this note reproduced exactly before drilling any further:

| anchor | value | asserted at |
|---|---|---|
| answers decomposed | 172 | `dx10_falsification.rs::s6_*` |
| result-frame bytes | 170,584 (171,220 after bn-27mx7 added recovery offers to stale-handle refusals) | `dx10_falsification.rs::s6_*` |
| request-frame bytes | 73,318 | `dx10_falsification.rs::s6_*` |
| `artifacts` | 20,732 | `dx10_falsification.rs::s6_*` |
| `omissions` | 16,656 | `dx10_falsification.rs::s6_*` |
| `assurance` | 15,720 | `dx10_falsification.rs::s6_*` |
| `epochs` | 8,084 | `dx10_falsification.rs::s6_*` |
| `SnapshotComponents` on the wire | 17,208 over 24 `workspace.create` | `dx10_falsification.rs::s6_*` |
| `next_operations`, `cost` | 0, 0 | `dx10_falsification.rs::s6_*` |
| native bytes per solved task | 10,859 | `pr10_impl02_interface_bytes.rs` |
| shell bytes per solved task | 4,118 | `pr10_impl02_interface_bytes.rs` |

**Scaling rule.** The decomposition is taken over the baseline's 172 recorded dispatches; the
typed arm makes 168 wire calls, because its register refuses four before the wire. A field
total `T` therefore becomes `T x 168 / 172 / 24` bytes per solved task — the same rule
`s6_where_the_typed_arms_bytes_go_and_how_far_a_redesign_reaches` applies. Divisor: 24.577.

---

## 2. The measured decomposition

### 2.1 Where the native arm's 10,859 B per solved task actually goes

Per-operation agent-visible bytes, both arms, over the whole matrix:

| operation | calls | native B | native/call | shell B | shell/call | ratio |
|---|---:|---:|---:|---:|---:|---:|
| `workspace.create` | 24 | 35,304 | 1,471 | 9,000 | 375 | 3.9x |
| `workspace.fork` | 8 | 12,658 | 1,582 | 4,240 | 530 | 3.0x |
| `workspace.seal` | 28 | 24,780 | 885 | 15,120 | 540 | 1.6x |
| `verification.start` | 36 | 37,248 | 1,034 | 27,328 | 759 | 1.4x |
| `task.status` | 48 | 78,144 | 1,628 | 25,704 | 535 | 3.0x |
| `verification.result` | 24 | 45,840 | 1,910 | 14,592 | 608 | 3.1x |
| `task.resume` | 4 | 5,656 | 1,414 | 2,852 | 713 | 2.0x |
| connection handshake | 24 | 21,000 | 875 | 0 | 0 | — |
| **total** | | **260,630** | 1,551 | **98,836** | 575 | 2.7x |

Two structural facts fall out of this table and both bind the rest of the note. First, the
typed arm pays a **875 B handshake per solved task** and the baseline pays none under the
landed rule — 8.1% of the typed arm's whole spend, and 30% of the entire byte budget a
passing row would allow. Second, the per-call ratio is **2.7x** and is remarkably flat: the
typed arm is not losing on one operation, it is losing on the shape of every exchange.

Result-frame field costs over the 172 answers (170,584 B):

| field | landed differential | full removal | class |
|---|---:|---:|---|
| `artifacts` | 20,732 | 23,312 | conformance-varying |
| `omissions` | 16,656 | 19,236 | conformance-varying |
| `assurance` | 15,720 | 15,720 | conformance-varying |
| `epochs` | 8,084 | **27,864** | conformance-constant |
| `next_operations` | 0 | 3,440 | amortizable |
| `cost` | 0 | 1,720 | conformance-varying |
| `payload` | — | 50,484 | — |

Two corrections to the ledger as recorded on `bn-762i` are visible here.

- **The `epochs` credit is understated by 3.4x.** 8,084 is the reduction to *protocol only*,
  which still ships six explicit nulls (105 B of the 152). Full removal is 27,864 B. This is
  not an academic difference: `ServerWelcome.epochs: EpochSet required` **already delivers
  the identical set at the handshake**, so the whole field is a re-send of a value the client
  held before its first call.
- **"`next_operations` and `Cost` cost ZERO" is true of their contents, not of their keys.**
  The two provably-empty members cost 5,160 B of key skeleton across the matrix — 210 B per
  solved task, more than a fifth of what the whole `epochs` field was credited with.

### 2.2 `artifacts` — 20,732 B at field grain (the commissioned drill)

144 `ArtifactRef` values over 172 answers, 20,724 B encoded standalone, mean 143.9 B.

**By kind** — two kinds, no others:

| kind | refs | bytes | share | shape |
|---|---:|---:|---:|---|
| `task` | 84 | 15,204 | 73.4% | `{"commitment":"task_<64 hex>","handle":"task_<64 hex>","kind":"task"}` (181 B) |
| `ws` | 60 | 5,520 | 26.6% | `{"handle":"ws_<64 hex>","kind":"ws"}` (92 B) |

**By member** (key, quotes, value and separator charged to the member):

| member | present on | bytes | share | note |
|---|---:|---:|---:|---|
| `handle` | 144 / 144 | 11,688 | 56.4% | value text 9,816 B, mean 68.2 B: a class prefix plus 64 hex characters |
| `commitment` | 84 / 144 | 7,224 | 34.8% | value text 5,796 B, mean 69.0 B; present on every `task` ref, on no `ws` ref |
| `kind` | 144 / 144 | 2,040 | 9.8% | **derivable from `handle`** — see below |
| `redacted` | 0 / 144 | 0 | 0% | never populated on this wire |
| braces | 144 | 288 | 1.4% | |

**Value versus reference.** Every member is already a reference: a handle, a content
commitment, a class token. Nothing in `artifacts` carries content. A "send it by reference
instead of by value" redesign therefore has *nothing to convert* — the field is already
by-reference, and its cost is the reference being sent again.

**`kind` is a substring of `handle`.** The declaration says so in its own doc comment
("Artifact class, the plan §4.4 prefix without the underscore") and `ArtifactHandle`'s
pattern `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` makes the prefix syntactically recoverable by any
reader. 2,040 B of the 20,732 is a field whose value can be computed from the field beside
it, on every answer, by both parties.

**Redundancy across the frames of one task.** Counting distinct `(kind, handle)` pairs
*within each run*:

| | count |
|---|---:|
| refs transmitted | 144 |
| distinct within their own run | **52** |
| refs naming an artifact the run already named | **92 (63.9%)** |
| distinct per run | 2–3, against 4–9 refs transmitted |

**By operation:**

| operation | answers | refs | bytes | share |
|---|---:|---:|---:|---:|
| `task.status` | 48 | 52 | 9,416 | 45.4% |
| `verification.start` | 36 | 24 | 4,344 | 21.0% |
| `workspace.seal` | 28 | 28 | 2,576 | 12.4% |
| `workspace.create` | 24 | 24 | 2,208 | 10.7% |
| `task.resume` | 4 | 8 | 1,452 | 7.0% |
| `workspace.fork` | 8 | 8 | 736 | 3.6% |
| `verification.result` | 24 | **0** | **0** | 0% |

Two observations that are not byte questions and are recorded here for whoever owns the RFC
correction rather than acted on by this note:

- **The answer that carries the semantic verdict names no artifact.** `verification.result`
  is the only operation in the matrix whose `artifacts` list is empty on every answer.
- **A poll carries the task handle three times.** On `task.status` the same 69-byte handle
  travels in `ResultEnvelope.task`, in `payload.task`, and in `artifacts[0].handle` — and the
  caller named it in the request that produced the answer.

### 2.3 `omissions` — 16,656 B

324 records over 100 of the 172 answers; 19,236 B on full removal; mean 51.4 B per record.

| operation | answers | records | bytes | manifest shape |
|---|---:|---:|---:|---|
| `verification.result` | 24 | 192 | 9,240 | `task.committed_evidence, Symbolic, Temporal, Probabilistic, Theorem, Runtime, crashpack, context` |
| `verification.start` | 36 | 72 | 3,888 | empty on refusal; else `task.committed_evidence, budget.wall_ms, budget.bytes` |
| `task.status` | 48 | 48 | 2,880 | `task.committed_evidence` |
| `task.resume` | 4 | 12 | 648 | `task.committed_evidence, budget.wall_ms, budget.bytes` |
| `workspace.*` | 60 | 0 | 0 | empty |

- **All 324 records carry `reason = "unsupported"`.** None is `budget`, `heuristic-cutoff`,
  or `slice-irrelevant`. An `unsupported` omission is a statement about what the *deployment*
  can meter or produce — a connection constant — charged on every answer that touches it.
- **`recoverable_by` is present on 0 of 324.** INV-007 requires an omission to name both what
  was left out and how to retrieve it; the retrieval half is never populated. Nothing on this
  wire is summarized-with-an-expansion-handle today, which is why the pattern RFC 0028 and
  RFC 0030 define has no byte cost to trade against yet.
- Ten distinct subjects, all constant strings; **four distinct manifest shapes across the
  entire matrix**, each a deterministic function of `(operation, admitted)`.

### 2.4 `assurance` — 15,720 B

- Present on 24 of 172 answers, only on `verification.result`; 642 B each.
- **All 24 are byte-identical.** One distinct value across the whole matrix.
- Six of nine dimensions are `unsupported` with a deployment-constant reason; three are
  `produced`, all three naming `continuum-engine-reference` — 1,872 B of repeated engine
  strings and 1,056 B of summaries across the matrix.
- What varies per campaign is the produced/unsupported partition: **nine bits**, carried in
  642 B.

### 2.5 `SnapshotComponents` on the request — 17,208 B

24 `workspace.create` requests, mean 717 B (Die Hard 718, Philosophers 660). Composition of
one request:

| part | bytes | note |
|---|---:|---|
| six or seven empty required lists | 117–136 | `cml_modules`, `correspondence`, `dependencies`, `domain_packs`, `proof_environment`, `rust_extraction` (+ `configuration` on Philosophers) |
| `files` | 150 | two commitments, **each already inside `file_components`** |
| `file_components` | ~200 | two `{commitment, path}` pairs — the normative shape |
| `epochs`, `intent`, `configuration` | ~230 | |

`rule snapshot.file_components` already fixes the relation ("`files` says *what*,
`file_components` says *where*"), and `file_components` carries both halves — so `files` is
derivable whenever `file_components` is present.

**The landed instrument over-credits this line.** `Decomposition::redesign_reducible` counts
the full 17,208 B as recoverable by "`SnapshotComponents` resolved by reference". On a fresh
connection creating a fresh snapshot the daemon has **no other channel** by which to learn
what the snapshot is; the reference has to be established before it can be used. Only the
trims are free without a new declaration, and the by-reference form needs a registered-port
mechanism that does not exist (§3, C4).

`workspace.fork` carries a further 6,130 B of arguments over 8 calls. That is overlay file
content — genuine payload, not envelope overhead, and not on this list.

### 2.6 `epochs` — 8,084 B credited, 27,864 B actual

One distinct encoding across all 172 answers, 152 B:

`{"corpus":"tla-examples-1","engine":"engine-reference-1","evidence":null,"intent":"intent-1","proof":"proof-1","protocol":"3.2","semantic":"semantic-1"}`

`ServerWelcome.epochs: EpochSet required` already carries this set at the handshake. Full
removal from the result envelope is 162 B per answer — 27,864 B over the matrix, **1,134 B
per solved task**, the largest single item in the entire ledger once measured correctly.

### 2.7 Two costs the ledger did not name

**(a) Content-digest re-transmission — 31,232 B.** Counting maximal runs of 32 or more hex
characters in the result frames:

| | value |
|---|---:|
| occurrences | 628 |
| bytes | 40,192 (**23.6% of all result bytes**) |
| distinct within their own run | 140 |
| first-mention bytes | 8,960 |
| **re-sent bytes** | **31,232 (77.7% of digest bytes)** |

**(b) `task.status`'s full record on every poll — 34,800 B.** 48 polls x 725 B of payload,
**69% of all payload bytes in the matrix**. Between two polls of one task only `status`,
`cost` and `continuation` change; `budget`, `committed_evidence`, `epochs` (a *fourth* copy
of the epoch set), `intent`, `milestones`, `operation`, `priority_class` and `snapshot` are
fixed for the task's life. `OutputPolicy.max_bytes` is declared "Enforced byte ceiling on the
result payload" and **no handler in `continuumd::daemon` enforces it** — the field is
accepted, keyed into the idempotency state, and ignored. *Erratum: that was the state at
measurement. `bn-6fuu5` landed enforcement on `task.status`. The 34,800 B measured here is
unmoved, because the instrument declares no ceiling and the summary that would take one is
falsified — §3 C7.*

---

## 3. Redesign candidates, one per ledger item

Every projected saving below is measured by re-encoding the recorded envelopes with the
transform applied in run order, not estimated. Candidates are applied cumulatively in the
order C1, C2, C3, C5, C7, C6 so overlapping savings are attributed once and never twice; the
"isolated" column is what the candidate is worth on its own.

Wire-visibility classes, per house doctrine: **behavior-only** changes ship with no protocol
bump; **declaration-moving** changes ride the *one* deferred-and-bundled protocol-minor (3.6)
raised at the end of the redesign, not one bump per bone.

### C1 — `artifacts`: first mention only, handle only

**Mechanism.** Three moves on one field.
1. Drop `kind`. It is the handle's own class prefix and is recoverable by any reader from the
   `ArtifactHandle` pattern. (2,040 B.)
2. Drop `commitment` from a ref whose handle this connection has already carried. The member
   is already `optional`, so this move alone is legal on today's wire. (Up to 7,224 B.)
3. Carry a ref only on the answer that **first** names its handle on this connection. A
   later answer about the same artifact names it in `ResultEnvelope.task` and in its payload
   already; the third copy carries no information the client does not hold.

**Projected saving.** 16,524 B over the matrix — **672 B per solved task**.

**Cost.** Zero extra round trips on all 24 tasks: by construction a repeat mention only
occurs on a connection that already received the full ref, and move 3 never elides a first
mention. A client that reconnects mid-task receives full refs again, because the "already
carried" set is connection-scoped.

**Wire-visibility.** Move 2 is **behavior-only**. Moves 1 and 3 are **declaration-moving**
(`ArtifactRef.kind` becomes optional-and-derived; a `rule artifacts.first_mention` fixes what
an elided repeat means) and ride the bundled 3.6.

### C2 — `omissions`: the `unsupported` set is a handshake fact

**Mechanism.** All 324 records say `reason = "unsupported"`, which is a statement about the
deployment's capability set, not about the answer. Declare that set once in `ServerWelcome`
(the frame that already carries `grant`, `limits`, `features` and `epochs`); an answer that
draws on it carries a single record naming the profile rather than the enumeration. This is
the summarize-with-expansion-handle shape RFC 0028 defines, with the handshake standing in
for the expansion round trip: the client holds the manifest before it makes its first call.

**Projected saving.** 12,456 B over the matrix — **506 B per solved task**.

**Cost.** Zero extra round trips: the profile is in a frame the client already has.
**Implementation obligation:** the instrument's policy reads `ceiling_enforced` as "no
omission names `budget.states`". Under C2 that predicate must be answered from the
handshake-delivered profile plus the answer's profile reference. A client that resolves it by
scanning the inline list only would silently read every ceiling as enforced. The redesign
bone must carry that predicate as an acceptance test.

**Wire-visibility.** **Declaration-moving** — a new `ServerWelcome` member and a rule fixing
what a profile reference means. Bundled 3.6.

### C3 — `assurance`: profile by reference, partition by value

**Mechanism.** All 24 assurance envelopes in the matrix are byte-identical. Six of nine
dimensions are `unsupported` with a constant reason and three name the same engine. Publish
the deployment's assurance *profile* — the nine dimensions with their producers and their
unsupported reasons — in `ServerWelcome`, and let an answer carry the profile reference plus
the produced/unsupported partition for *this* campaign. Plan B11 ("all nine dimensions
REQUIRED on every semantic verdict") is preserved in substance: every dimension is still
named, in a frame the client holds, and the answer still states which were established.

**Projected saving.** 13,944 B over the matrix — **567 B per solved task**.

**Cost.** Zero extra round trips.

**Risk to state plainly.** This is the candidate closest to weakening a guarantee. B11's
force is that a verdict cannot be read without its assurance; a by-reference form makes that
readable only by a client that kept the welcome. The redesign bone must therefore carry a
conformance test that a verdict answer is *unreadable* — a typed error, never a default —
to a client that cannot resolve the profile.

**Wire-visibility.** **Declaration-moving.** Bundled 3.6. Shares the welcome-extension
mechanism with C2; design them together.

### C4 — `SnapshotComponents`: trim the derivable, then reference the port

**Mechanism, safe half (C4a).** Drop `files` when `file_components` is present — `rule
snapshot.file_components` already makes it derivable — and let the six-or-seven empty
required lists be absent rather than empty.

**Mechanism, aggressive half (C4b).** Accept a `workspace.create` that names a **registered
corpus port by its content commitment** instead of enumerating its components. The port is
already content-addressed (RFC 0019), so this is by-reference in the strict sense; it is
*not* available today and needs a new request form.

**Projected saving.** C4a 6,408 B = **260 B per solved task**. C4b 12,888 B = **524 B per
solved task** (subsumes C4a).

**Cost.** Zero extra round trips for C4a. C4b costs one registration exchange per *port*, not
per task — amortized to zero across the instrument's 24 tasks over 2 ports, and to one extra
call for a client creating a snapshot from components no daemon has seen.

**Wire-visibility.** Both halves **declaration-moving** (required lists become optional; a
new request form for C4b). Bundled 3.6.

**Correction this candidate carries.** `Decomposition::redesign_reducible` credits the full
17,208 B to "components resolved by reference" without a mechanism that establishes the
reference. C4b is that mechanism and it still does not recover the whole line, because the
intent handle and the snapshot epochs must travel. Whoever owns the RFC 0027 correction
should record that the landed reducible figure is an upper bound, not an available saving.

### C5 — `epochs`: pinned at the handshake, absent when unchanged

**Mechanism.** `ServerWelcome.epochs` already carries the connection's epoch set. Make
`ResultEnvelope.epochs` `optional`, with absence meaning "the set the welcome pinned,
unchanged", and require it present on any answer whose epochs differ — which is exactly the
case ADR-0018 and plan §4.6 care about (a continuation resuming under a pinned older epoch).
The guarantee is preserved exactly: an epoch is still named on every answer, and every change
is still stated.

**Projected saving.** 27,864 B over the matrix — **1,134 B per solved task.** The largest
single item in the ledger.

**Bundle with it:** make the two provably-empty required members (`next_operations`, `cost`)
absent rather than empty — 5,160 B of key skeleton, a further **210 B per solved task**.

**Cost.** Zero extra round trips.

**Wire-visibility.** **Declaration-moving** — three `required` markers become `optional`, plus
a rule fixing what absence means. Bundled 3.6. No new member anywhere.

### C6 — connection-scoped handle aliases

**Mechanism.** A content identity is transmitted 628 times in the result frames and only 140
of those are first mentions on their connection. At first mention the full handle travels; a
later mention on the same connection carries the class prefix and a short connection-local
ordinal. The alias table is derived by both parties from frames they have both already seen,
so nothing negotiates it.

**Projected saving.** 16,104 B **net of C1 and C5** (which remove many occurrences first) —
**655 B per solved task**. Measured in isolation it is worth 33,184 B, which is why it must be
sequenced after C1 and C5 and never added to their totals.

**Cost.** Zero extra round trips.

**Hard constraint.** Aliases are connection state, and INV-004's "a certificate is checked
from its wire form" forbids connection state from reaching a published artifact. Aliasing
applies to **envelope-level references only** — never inside a `payload` that is stored,
published, or checked, and never inside a commitment. The redesign bone must carry that as a
conformance test, not a comment.

**Wire-visibility.** **Declaration-moving** (handles gain an alias form and a rule binding
it). Bundled 3.6.

### C7 — `task.status`: honor `OutputPolicy`, summarize with an expansion route — FALSIFIED BY MEASUREMENT

**Mechanism.** `OutputPolicy.max_bytes` is already declared "Enforced byte ceiling on the
result payload" and no handler enforces it. Enforce it: when the ceiling binds, return the
members the caller needs (`task`, `status`, `cost`, `continuation`) and record the remainder
as an INV-007 omission whose `recoverable_by` names the task handle — which is the retrieval
route, and which fills a field that is populated 0 times out of 324 today. This is the
summarize-with-expansion-handle pattern of RFC 0028 applied to a task record, riding
declarations that already exist.

**Projected saving.** 27,376 B over the matrix — **1,114 B per solved task**, the second
largest item and the only one that needs no protocol bump at all.

**Cost.** One extra round trip for a client that needs the full record from a poll. **On the
instrument's task mix that is zero of 24 tasks**: the policy reads `task`, `status`,
`cost.states` and `continuation` from a poll and nothing else, and all four are retained.
Break-even is roughly **one expansion per two summarized polls** — the summary saves 570 B
per poll and an expansion costs about 1,100 B post-redesign — so the move is a net loss only
for a client that needs the full record on more than half its polls.

**Wire-visibility.** **Behavior-only. No bump.** Both `OutputPolicy.max_bytes` and
`Omission.recoverable_by` are declared today; honoring them is conformance, not redesign.
This candidate can and should land before the bundled 3.6 exists.

**ERRATUM — FALSIFIED BY MEASUREMENT.** `bn-6fuu5`, 2026-08-09. Everything above this line
stays as the record of what was projected. Both of its headline claims are wrong: the
mechanism is not behavior-only, and the saving is not positive.

- **Not behavior-only — it is a MAJOR.** An answer carrying `task`, `status`, `cost` and
  `continuation` and nothing else is an answer without `operation`, `snapshot`, `intent`,
  `epochs`, `priority_class`, `budget`, `milestones` and `committed_evidence`. `TaskRecord`
  declares six of those `required` and two `nullable`. `rule versioning.breaking_change`
  lists "changing a field's type or presence marker" among the changes that MUST advance the
  MAJOR and publish the plan §4.6 per-artifact-class compatibility statement. The summary
  mechanism is therefore not a 3.6-minor item either: it cannot ride the bundled minor with
  items 2-7, and it cannot land first. This correction is larger than the byte number,
  because the whole seven-item set was accepted on the record that item 1 needed no
  declaration to move.
- **The measured saving is negative.** `bn-6fuu5` re-ran this note's own method — recorded
  envelopes re-encoded with `continuumd`'s codec, driven through landed enforcement code
  rather than a model of it — over the matrix's 48 `task.status` answers, 35,368 B of
  payload. Best case, taking each poll's optimum ceiling independently and so an optimum no
  real caller can pick, the set loses **1,196 B over the matrix = −49 B per solved task**. At
  the tightest conforming ceiling, which is the summary this candidate describes, it loses
  **6,128 B = −255 B per solved task**. The counterfactual that does reach +1,114 has to be
  built by string surgery, because the Rust type cannot express the absence, and every byte
  of it is a presence marker moving.
- **Why it loses.** What the declaration leaves a ceiling to take is the contents of two
  `required` lists plus the nine `optional` members of `Budget`: **122-289 B per poll**. A
  conforming INV-007 record carrying `recoverable_by` measures **136 B**, 47 B without it,
  and `recoverable_by`'s value is a **fourth** copy of the same 69-byte task handle the
  answer already carries three times — §2.2. Stating the ceiling costs a further 34 B on the
  request frame. Populating INV-007's retrieval half for the first time on this wire, which
  is the reason this candidate was ordered first, is exactly what makes the trade lose. The
  projected 1,114 charged neither the omission record nor the request-side declaration.
- **What survives, and is landed.** `OutputPolicy.max_bytes` was declared "the enforced
  contract" and read by no handler. It is now enforced on `task.status` with three typed
  dispositions — answered whole, shortest-prefix elision with an INV-007 omission naming the
  retrieval route, or a typed refusal when nothing conforming fits. That is conformance, it
  moves no declaration, and it stays. The instrument declares no ceiling, so the landed
  native figure is unchanged at 10,859 B per solved task and no control moved.
- **Re-scoped to the major horizon.** The `task.status` summary is deferred to the MAJOR
  horizon. It is not in the 3.6 bundle, it is no longer this note's item 1, and it
  contributes **0 B** to the corrected set total in §4.

---

## 4. The sum-check

Target, stated explicitly. The ratified margin is +30% and the baseline spends **4,118 B per
solved task**, so a passing native arm must land at

> **native <= 4,118 x 0.70 = 2,882 B per solved task**

which from today's 10,859 requires giving back **7,977 B per solved task**, or **73.5% of the
typed arm's entire wire spend**.

| candidate set | saved (matrix) | saved / solved | native after | margin |
|---|---:|---:|---:|---:|
| landed measurement | — | — | 10,859 | **−163%** |
| the five commissioned ledger items (C1, C2, C3, C4b, C5) | 83,676 | 3,405 | 7,454 | **−81%** |
| + the two discoveries (C6, C7) | 127,156 | 5,174 | **5,685** | **−38%** |
| + the empty-member skeleton (C5 bundle) | 132,316 | 5,384 | 5,475 | −33% |
| landed instrument's "maximal encoding redesign" | — | — | 7,668 | −86% |
| **required for the ratified floor** | — | **7,977** | **<= 2,882** | **+30%** |

**Verdict: the candidates cannot clear the margin, and neither can any redesign of this
encoding.** The five commissioned items, realised in full and with no guarantee dropped,
recover 3,405 B of the 7,977 B needed — 43% of the gap. Adding both discoveries reaches
5,174 B, 65% of the gap, and still lands at −38%. This finding goes to the lead **before**
implementation is commissioned, which is what `bn-3bn62` asked for.

Note that the candidate set already beats the landed instrument's own "maximal encoding
redesign" bound (5,685 against 7,668): it reaches past that bound because it also cuts the
payload and the digest re-transmission, which the landed maximal did not model. It is
therefore a genuinely better redesign than the falsification campaign's floor assumed — and
it still fails, by a wider margin than the shortfall it closed.

### Erratum to the sum-check — the corrected total

`bn-6fuu5` falsified C7 after the table above was written; see the erratum in §3. C7
contributes nothing, so the set is the **six-item 3.6 set** — C1, C2, C3, C4b, C5 and C6.
The rows above stay as the record of what was projected. These are the numbers that hold:

| candidate set | saved over the matrix | saved / solved | native after | margin |
|---|---:|---:|---:|---:|
| landed measurement | — | — | 10,859 | **−163%** |
| the five commissioned ledger items C1, C2, C3, C4b, C5 | 83,676 | 3,405 | 7,454 | **−81%** |
| **+ C6, the one surviving discovery — the six-item 3.6 set** | **99,780** | **4,060** | **6,799** | **−65%** |
| + the empty-member skeleton bundled with C5 | 104,940 | 4,270 | 6,589 | **−60%** |
| C7, withdrawn — measured −49 to −255 B per solved task | **0** | **0** | — | — |
| landed instrument's "maximal encoding redesign" | — | — | 7,668 | −86% |
| **required for the ratified floor** | — | **7,977** | **<= 2,882** | **+30%** |

Restated at the corrected total. The six-item set recovers **4,060 B of the 7,977 B per
solved task the ratified floor needs — 51% of the gap**, not 65%, and lands the native arm
at **6,799 B per solved task**, a **−65%** margin. With the empty-member skeleton bundled
into C5 it reaches 4,270 B, 54% of the gap, and −60%. The set removes **37.4% of the native
arm's total wire spend**, not 47.6%. The shortfall is **3,917 B per solved task**, not 2,803.

**The verdict is unchanged and stronger.** The candidates cannot clear the margin, and
neither can any redesign of this encoding. Two secondary readings do change. The set no
longer beats the landed instrument's own "maximal encoding redesign" bound by as much —
6,799 against 7,668 rather than 5,685 against 7,668 — and it is still past that bound
because it cuts the payload and the digest re-transmission the landed maximal did not model.
And the zero-round-trip claim is now unconditional: C7 was the one candidate whose cost was
an extra round trip for a client that needs the full record from a poll, so with it withdrawn
the six-item set costs no extra round trip for **any** client, not merely none on this mix.

### The round-trip lever, tested and rejected

The obvious remaining move is fewer exchanges rather than smaller ones, and the protocol
already supports it: `verification.await` is declared in `continuumd::protocol::registry`
(`@readonly @task_starting`, response `VerificationResult`) and implemented in the daemon,
and `workspace.create` already takes `seal`. Collapsing `start` + `poll*` + `result` into one
`await`, and `create` + `seal` into one call, takes the matrix from 172 dispatches to 60.

Applied to the typed arm alone and combined with every candidate above, that lands at 3,072 B
per solved task — a +25% margin, which *looks* like it nearly clears the floor. C7's
withdrawal does not move this figure: the compound sequence collapses `start` + `poll*` +
`result` into one `await`, so the `task.status` polls C7 acts on are not in it.

**It does not, and the reason must be recorded so nobody re-derives the mistake.** The
compound sequence is available to the baseline too — a CLI projects the same operations — and
when both arms use it:

| arm | before | compound | change |
|---|---:|---:|---|
| native | 10,859 | 5,019 | −54% |
| shell | 4,118 | 1,809 | −56% |
| **margin** | **−163%** | **−177%** | **worse** |

The margin gets *worse*, because the typed arm's fixed 875 B handshake does not shrink with
the call count while everything else does. Charging the typed arm a compound sequence and the
baseline the old one would have been exactly the accounting move the `bn-2c0a` campaign
warned against, and it is refused here.

---

## 5. What would clear the margin, and what the lead must decide

The condition is algebraic. With `H` the handshake charged per task, `n` calls per task, `rn`
the typed arm's bytes per call and `rs` the baseline's:

> `H + n·rn <= 0.7 · n · rs`

On the instrument's measured values (`H` = 875, `n` = 7, `rs` = 575) this requires

> **`rn` <= 277 B per call, request and result, both directions.**

Today `rn` = 1,551. After the full candidate set `rn` = 679. The candidate set therefore
travels 68% of the distance from 1,551 to 277 and stops. What remains at 679 B per call is
the request envelope, the response skeleton, and the payload — which is what the protocol
*is*. The structural statement:

> The baseline's 575 B per call is a **rendering** — a lossy prose summary of an answer. The
> typed arm's is a **lossless, self-describing encoding** of the same answer plus the request
> that asked for it. The ratified margin asks the lossless encoding to be less than half the
> size of a lossy summary of itself. No redesign that keeps the protocol lossless can satisfy
> it; a redesign that does satisfy it has become a rendering with extra steps, and has thrown
> away the omission manifests, epoch pinning, assurance envelope and artifact identity that
> are the typed surface's entire reason to exist.

Four routes remain, and none of them is this bone's to take:

1. **Amortize the handshake across tasks.** One connection per agent session rather than per
   task takes 875 B to 219 B at four tasks — 656 B per solved task, more than any single
   candidate here. This is an *instrument* change (`bn-2phq3` owns the harness), and it sits
   directly on top of the pro-shell handicap attack S2 already landed: the landed rule
   charges the baseline no handshake at all. Raising it re-opens an accounting question.
2. **Negotiate `canonical_cbor`.** It is already a member of the closed `Encoding` enum,
   already listed in `registry::ENCODINGS`, and already implemented (`codec::to_cbor_bytes`);
   the benchmark's `hello()` offers only `canonical_json`. Measured on the envelope minus its
   payload, **CBOR is 83% of canonical JSON** — 20,068 B over the matrix, about **816 B per
   solved task**, behavior-only, no bump. **This note does not recommend it.** It shrinks the
   frames the pinned accounting charges without changing one bit of what the agent learns,
   which is the same species of move as choosing the accounting that flips the sign. If it is
   taken, it should be taken as an explicit, recorded decision by whoever owns the metric's
   meaning — not folded into a redesign bone.
3. **Re-ratify the margin, or re-open X1.** Both sit above this bone, and X1 was adjudicated
   on 2026-08-09 and is closed.
4. **Read the exit sentence as written.** "Native ACI is measurably more effective **or** the
   protocol is redesigned before freeze." It is a disjunction, and the ruling already took the
   second branch. On that reading the redesign's obligation is *not* to flip the bytes row —
   it is to remove the waste the instrument demonstrated. Against that obligation this
   candidate set removes **47.6% of the typed arm's total wire spend**, drops **no protocol
   guarantee**, and costs **zero extra round trips on all 24 solved tasks of the instrument's
   mix**. The G0-DX-10 bytes row will still read as measured; the exit is satisfied by the
   redesign having happened, and the honest artifact is this note plus the re-run.

**Erratum to this section, `bn-6fuu5`, 2026-08-09.** C7 is withdrawn, so two figures above
move and neither conclusion does. The demonstrated post-redesign floor is **above** the 679 B
per call quoted here, by C7's withdrawn 27,376 B spread over the matrix, and the six-item set
therefore travels **less** than 68% of the distance from 1,551 to 277 before it stops on the
request envelope, the response skeleton and the payload. Route 4's set figure is **37.4% of
the typed arm's total wire spend**, not 47.6% — §4 carries the corrected arithmetic — and the
zero-round-trip half of that sentence gets stronger rather than weaker, because C7 was the
one candidate that could cost a round trip and it is gone. The structural statement, the
unreachability finding and the recommendation are unchanged.

**Recommendation to the lead:** commission the candidate set on route 4, and record in the
RFC 0027 correction that the +30% bytes margin is **unreachable under the pinned accounting
by any lossless protocol**, with the algebra of this section as the reason. Do not commission
work whose stated goal is a passing bytes row; nothing in this ledger can deliver one.

---

## 6. Recommended implementation order

| # | candidate | bone | B/solved | bump | why here |
|---|---|---|---:|---|---|
| 1 | **C7** `task.status` summary under an enforced `OutputPolicy` | `bn-6fuu5` | 1,114 | none | the only conformance fix in the set; ships before the bundled minor exists and needs nothing from the others |
| 2 | **C5** epochs pinned at the handshake, plus empty-member skeleton | `bn-2in7i` | 1,344 | 3.6 | largest single item; one `required` → `optional` flip, the welcome already carries the value |
| 3 | **C1** `artifacts` first-mention/handle-only | `bn-1xy9r` | 672 | 3.6, move 2 is free | must precede C6 so the overlap is attributed once |
| 4 | **C2** omission profile in `ServerWelcome` | `bn-1sxds` | 506 | 3.6 | establishes the welcome-profile mechanism C3 reuses |
| 5 | **C3** assurance profile by reference | `bn-2iyci` | 567 | 3.6 | reuses C2's mechanism; carries the B11 conformance test |
| 6 | **C6** connection-scoped handle aliases | `bn-h7vn7` | 655 | 3.6 | measured net of C1 and C5, so it lands after both |
| 7 | **C4** `SnapshotComponents` trim, then port-by-reference | `bn-2ug29` | 260 / 524 | 3.6 | request-side, independent of everything above |
| — | **one bundled protocol-minor 3.6** | — | — | — | raised once, at the end, covering items 2–7 |

Dependencies recorded in the tracker: `bn-1sxds` blocks `bn-2iyci`; `bn-1xy9r` and `bn-2in7i`
both block `bn-h7vn7`. Everything else is independent.
C7 is deliberately first because it is the only item that delivers a measured saving with no
declaration change, and because populating `Omission.recoverable_by` for the first time on
this wire is what makes the expansion protocol a real trade rather than a design intention.

**Erratum to the order, `bn-6fuu5`, 2026-08-09 — item 1 is withdrawn and the reason it was
first is the reason it fails.** The table above stays as the record of what was ordered. C7
is falsified by measurement, so it leads nothing and rides nothing; see the erratum in §3.
Populating `Omission.recoverable_by` for the first time on this wire is what makes the trade
lose, because that record costs 136 B per poll against a 122-289 B ceiling, and the summary
that would pay for it moves presence markers `rule versioning.breaking_change` makes a MAJOR.
The corrected order is the six 3.6 items, and **C5 under `bn-2in7i` now leads**:

| # | candidate | bone | B/solved | bump | why here |
|---|---|---|---:|---|---|
| 1 | **C5** epochs pinned at the handshake, plus empty-member skeleton | `bn-2in7i` | 1,344 | 3.6 | largest single item; one `required` → `optional` flip, the welcome already carries the value |
| 2 | **C1** `artifacts` first-mention/handle-only | `bn-1xy9r` | 672 | 3.6, move 2 is free | must precede C6 so the overlap is attributed once |
| 3 | **C2** omission profile in `ServerWelcome` | `bn-1sxds` | 506 | 3.6 | establishes the welcome-profile mechanism C3 reuses |
| 4 | **C3** assurance profile by reference | `bn-2iyci` | 567 | 3.6 | reuses C2's mechanism; carries the B11 conformance test |
| 5 | **C6** connection-scoped handle aliases | `bn-h7vn7` | 655 | 3.6 | measured net of C1 and C5, so it lands after both |
| 6 | **C4** `SnapshotComponents` trim, then port-by-reference | `bn-2ug29` | 260 / 524 | 3.6 | request-side, independent of everything above |
| — | **one bundled protocol-minor 3.6** | — | — | — | raised once, at the end, covering all six |
| — | **C7** `task.status` summary | `bn-6fuu5`, enforcement half only | 0 | MAJOR | deferred to the major horizon; the landed half is `OutputPolicy` enforcement, which moves no declaration |

The dependencies are unchanged, and the set no longer has an item that ships before the
bundled minor exists. Six items, one bump, 4,270 B per solved task with the C5 skeleton
bundled — §4 for the arithmetic.

---

## 7. What this note does not do

- **It does not decide G0-DX-10.** The row's verdict belongs to `bn-762i`; §4's sum-check is
  evidence for that bone, not a substitute for it.
- **It does not change the instrument.** Every number is read out of the landed harness;
  `bn-2phq3` owns any harness change, including handshake amortization and encoding
  negotiation.
- **It does not correct RFC 0027 or the matrix.** The two corrections this decomposition
  found — that the `epochs` credit is understated 3.4x and that the `SnapshotComponents`
  credit is an upper bound rather than an available saving — are recorded here for the bone
  that owns the RFC and the matrix row.
- **It measures one task mix.** Four tasks, two families, three schedules, two policies, a
  scripted policy that never discovers a strategy. The round-trip costs quoted for C7 and C4b
  are computed against *this* mix; a client population that reads full task records or builds
  snapshots from unregistered components pays more, and the break-evens in §3 are where to
  check that.

---

## 8. Round-2 wire-visibility pre-check — C1, C2, C3, C4, C6

**Amendment, `bn-d2bdi`, 2026-08-09.** Nothing above this heading is rewritten. §3's mechanisms
and §4's and §6's arithmetic stay as the record of what was projected; this section states what
each mechanism's own declaration permits, and it changes the *version* at which the projections
are collectable, not the projections.

**Why it exists.** Two consecutive items had their wire-visibility class falsified in the same
direction — C7 by `bn-6fuu5`, C5 by `bn-2in7i` — and both fell on the same rule: a mechanism
that suppresses or restructures a member moves a presence marker, which
`rule versioning.breaking_change` makes a MAJOR. The remaining five were classified by the same
method and none had been checked against that rule. `bn-2in7i`'s recommendation was to check all
five before any was commissioned. This is that check.

**Label note.** The bone text that commissioned this section swapped two labels. The ledger's
labels govern and the bone IDs disambiguate: **C4 is `SnapshotComponents`, `bn-2ug29`**, and
**C6 is connection-scoped handle aliases, `bn-h7vn7`**.

### 8.1 Method

Three steps per item, and the third is what makes a verdict rather than a reading.

1. **Quote the declaration.** `notes/plan/schemas/continuumd-native-protocol.idl` is the verdict
   source — the member, its presence marker, its owning struct and its line.
2. **Apply both rules in full.** `rule versioning.compatible_change` at line 650 opens "Within a
   protocol major, only compatible changes are permitted, and each MUST raise the minor version"
   and then names exactly five: adding an operation; adding an `optional` field; adding an enum
   member to an `@open` enum; relaxing a server-side constraint; adding an error code. **A
   mechanism that is none of the five is not permitted within major 3 at all** — the rule is a
   closed list, not a set of examples. `rule versioning.breaking_change` at line 662 names what
   must advance the major, "changing a field's type or presence marker" first among them, and
   requires the plan §4.6 typed `Preserved | Revalidate | Incompatible` statement **before** the
   change is applied.
3. **Take the verdict mechanically.** Where presence is the question, a real recorded value is
   re-encoded with `continuumd`'s own codec, the member is removed by `variants::without_member`
   — the tested canonical-JSON surgery `bn-2in7i` built — and the frame is fed back to the
   decoder **this repo generates from that declaration**. What the decoder does is the verdict.
   Measurements below were taken over the same 172-answer recording sweep as §2, and the landed
   anchors were reproduced first: 172 answers, 170,584 result B, 17,208 B of
   `SnapshotComponents` over 24 `workspace.create` calls, 144 `ArtifactRef` values, 324 omission
   records, 628 digest occurrences at 40,192 B.

### 8.2 The verdict table

| item | bone | mechanism as specified | declaration it touches | verdict | 3.6 B/solved |
|---|---|---|---|---|---:|
| **C1** | `bn-1xy9r` | drop `kind`; drop `commitment` on a repeat; carry a ref on first mention only | `ArtifactRef.kind: String required` — IDL 1421 | **major-gated** | **0** |
| **C2** | `bn-1sxds` | omission profile in `ServerWelcome`, one record in place of the enumeration | `ResultEnvelope.omissions: list<Omission> required` — IDL 1659; `enum OmissionReason` closed — IDL 1183 | **eligible-with-modified-mechanism, worth 0 B** | **0** |
| **C3** | `bn-2iyci` | assurance profile by reference, partition by value | `AssuranceEnvelope` nine members `required` — IDL 1407-1415 | **eligible-with-modified-mechanism, worth 0 B** | **0** |
| **C4a** | `bn-2ug29` | drop `files`, let the empty required lists be absent | `SnapshotComponents.files: list<Commitment> required` — IDL 2653, and eight peers | **major-gated** | **0** |
| **C4b** | `bn-2ug29` | `workspace.create` naming a registered corpus port by commitment | no declaration moves — a **new operation** under `compatible_change` clause 1 | **eligible-with-modified-mechanism, and it is the only survivor** | **494** |
| **C6** | `bn-h7vn7` | class prefix plus a connection-local ordinal after first mention | `rule handshake.no_session_state` — IDL 2124; `rule versioning.handle_stability` — IDL 708 | **major-gated, and blocked by INV-002 above the version question** | **0** |

Two rows carry a shape the C7 precedent named and this pre-check found twice more: a mechanism
whose conforming version is admissible and **worth nothing**. C7's conforming ceiling measured
−49 to −255 B per solved task; C1's move 2 measures **−236 B over the matrix**; C2's and C3's
conforming halves measure **exactly 0 B**. An "eligible-with-modified-mechanism" verdict is not
a saving until the modified mechanism is priced, and three of the four here price at zero or
below.

### 8.3 C1 — `artifacts` first-mention and handle-only — MAJOR-GATED

**Declarations.**

```text
/// A typed artifact reference in a result.
struct ArtifactRef {                                    // IDL 1419
  /// Artifact class, the plan §4.4 prefix without the underscore.
  kind: String required;                                // IDL 1421
  handle: ArtifactHandle required;                      // IDL 1422
  /// Content commitment, when the artifact is content-addressed.
  commitment: Commitment optional;                      // IDL 1424
  redacted: Redacted optional;                          // IDL 1426
}
```

**Move 1, drop `kind` — MAJOR.** `kind` is `required`. Taken mechanically: a real recorded ref,
re-encoded and stripped, comes back
`CodecError::MissingField { declared_by: "ArtifactRef", field: "kind" }`. RFC 0026 also polices
this member twice by hand — its result-envelope table reads "typed refs
`kind`, `handle`, `commitment?`, `redacted?`, **not bare handles**", and its correction 25
corrects docs/36 for spelling a `kind` value wrongly. A member the RFC corrects other documents
about is not a member a minor may delete. **Measured worth: 1,896 B over the matrix, 77 B per
solved task** — see the erratum in §8.7, the ledger's 2,040 is 1 B per ref too high.

**Move 2, drop `commitment` on a repeat — admissible-shaped and NEGATIVE.** The member is
`optional`, so the decoder accepts its absence and this is the one move in the whole set that
moves no marker. It fails on two other grounds and the second is measured.

- *It states something false.* The declaration conditions absence — "Content commitment, when
  the artifact is content-addressed". On today's wire `commitment` is present on all 84 `task`
  refs and absent on all 60 `ws` refs, so absence already means "not content-addressed". Making
  it also mean "you already have it" is the same species of move as writing null over a pinnable
  epoch, which `rule envelope.epochs_named` forbids and which `bn-2in7i` refused. RFC 0026's
  redaction section states the general principle outright: "a client MUST be able to tell
  'withheld' from 'absent' structurally, without inference."
- *Conforming, it loses.* INV-007 requires a deliberate omission to be recorded. The cheapest
  record the codec will encode measures **58 B** and the elided commitments measure **5,100 B
  over 92 repeat refs**, so the conforming form is **−236 B over the matrix**. It cannot even be
  spelled: `enum OmissionReason` at IDL 1183 is **closed** with five members — `budget`,
  `redaction`, `unsupported`, `heuristic-cutoff`, `slice-irrelevant` — none of which means
  "already carried", and `rule versioning.breaking_change` makes adding a member to a closed enum
  a MAJOR.

**Move 3, first mention only — MAJOR, and its premise is false on this deployment.** The list
stays `required` and present, so the decoder accepts a shorter one; the gate is elsewhere, and
this is the finding worth carrying forward:

> **8 of the 92 repeat refs are not byte-identical to their first mention.** They carry a
> *different* `commitment` under the *same* `handle`, on `task.status` and `task.resume` — a
> task's content identity advances as the task does. C1's premise, "the third copy carries no
> information the client does not hold", is false on 8.7% of the repeats it would elide.

That is C5's hazard exactly, and worse: `bn-2in7i` found 172 of 172 epoch sets identical, so
suppression there would have elided only repetition. Here suppression would elide a **change**,
on this very mix, with no rule requiring the daemon to notice. Restricted to the 84 genuinely
byte-identical repeats the move is worth **12,440 B, 506 B per solved task** — and it still
needs `rule artifacts.first_mention` to fix what an elided repeat means, which is none of
`compatible_change`'s five and therefore not permitted within major 3. RFC 0026 additionally
requires the list to be complete in the one case it discusses — "A redacted artifact still
appears in `artifacts` with its `kind` and `handle`; it is not removed from the list" — and its
existence-oracle section refuses to let the list vary with what the daemon has already sent: "A
dedup that is invisible in `artifacts` but visible in reported cost is still an oracle."

**One theory checked and refused.** `commitment` is not a second spelling of `handle` the way
`kind` is a prefix of it: **0 of 84** task refs carry a commitment equal to their handle. There
is no third derivable member here.

**C1 at 3.6: 0 B.** At a major, moves 1 and 3 together are worth **14,336 B, 583 B per solved
task**, against §3's projected 672.

### 8.4 C2 and C3 — the two `ServerWelcome` profiles — ELIGIBLE-WITH-MODIFIED-MECHANISM, WORTH 0 B

Both split the same way, and the split is the point: **the welcome half is a compatible change
and the answer half is not.**

**The welcome half is clean.** `ServerWelcome` at IDL 1878 declares nine members, all `required`;
adding a **tenth as `optional`** is `compatible_change` clause 2 verbatim, and it is how every
member ever added to this protocol arrived — `ResultEnvelope.audit` at 3.1,
`SnapshotComponents.file_components` at 3.2, `CapabilityDescriptor.profile` at 3.1. A 3.6 daemon
may publish an omission profile and an assurance profile in the welcome today.

**C2's answer half is not.** The saving is not in the welcome, it is in replacing the
enumeration:

```text
  /// The INV-007 omission manifest. Empty list means nothing was
  /// omitted; the field is never absent.
  omissions: list<Omission> required;                   // IDL 1659
struct Omission {                                       // IDL 1430
  reason: OmissionReason required;                      // IDL 1431
  subject: String required;                             // IDL 1434
  recoverable_by: ArtifactHandle optional;              // IDL 1436
}
```

Measured: **324 records over 100 answers, 16,756 B; collapsing each manifest to one record saves
10,656 B, 434 B per solved task** — against §3 C2's projected 12,456 B and 506, an erratum of
1,800 B in the ledger's disfavour. Three separate blocks, any one of them decisive.

- **`OmissionReason` is closed.** Five members, `OPEN = false`, asserted structurally. No member
  means "see the profile", and adding one is a MAJOR.
- **`subject` is declared "What was omitted, in typed terms — never free-form prose".** A profile
  reference is not what was omitted; it names where to look it up. Making `subject` carry it
  changes what an existing required member means, which is none of the five.
- **A shorter manifest is a shorter statement.** `rule context.compilation` clause 3 already
  fixes the relation for one operation — "The envelope's `omissions` list is the wire projection
  of the pack's manifest and MUST agree with it **record for record**" — and INV-007 is that
  requirement generalized. The plan's own INV-007 delivery evidence is the same decoder-strip
  this section uses: "a real `ResultEnvelope.omissions` cannot be dropped from the wire".

**C3's answer half is not, and for a sharper reason.** `ResultEnvelope.assurance` at IDL 1650 is
`optional`, so — measured — the decoder **accepts** an answer with no assurance at all, and
removing it saves 655 B on each of the 24 carriers, the 15,720 B of §2.4 exactly. The gate is
not the decoder, it is `rule envelope.assurance_required` at IDL 1748: "A result whose `verdict`
is `semantic` or `evaluation` MUST carry the nine-dimension `assurance` envelope … Hiding
uncertainty to save tokens is prohibited." And inside it there is no room: all nine members of
`AssuranceEnvelope` at IDL 1407-1415 are `required`, and stripping **any one of the nine** from
a real recorded envelope returns `MissingField`. A form carrying "the profile reference plus the
partition" is either a new type on `assurance` — "changing a field's type", the second thing
`breaking_change` names — or `assurance` absent on a verdict, which the rule forbids. Rewriting
that rule is the C5 shape: `bn-2in7i` had to put "rewrite `rule envelope.epochs_named`" on its
4.0 checklist for the same reason.

**Modified mechanism, stated precisely, for both:** *publish the profile in `ServerWelcome` as a
new `optional` member and change nothing in the answer.* That is admissible at 3.6 and its
measured saving is **0 B**, because every byte of C2 and C3 is in the answer. It is worth landing
only if a later major will collect on it; on its own it adds bytes to the handshake, which §2.1
already shows is the typed arm's worst line at 875 B per solved task.

### 8.5 C4 — `SnapshotComponents` — C4a MAJOR-GATED, C4b THE ONE SURVIVOR

**C4a is refused by the IDL in its own words.** Every member the trim touches is `required` —
`files` at IDL 2653 and eight peer lists, `epochs` at 2661, `intent` at 2664 — and stripping each
from a real request returns `MissingField`. The measurement confirms §3's C4a exactly: `files`
150 B and six empty required lists 117 B per request, **267 B x 24 = 6,408 B**, §3's figure to
the byte. But the decisive citation is not the marker, it is that
`rule snapshot.file_components` at IDL 2698 **already answered this question and answered no**:

> The field is `optional` rather than replacing `files`, because changing that field's type is a
> breaking change under `rule versioning.breaking_change` and this revision is a minor.

RFC 0026's correction 43 records the same decision. C4a proposes at 3.6 the exact move 3.2
considered and declined, and nothing has changed since.

**C4b is eligible, in a modified form, and it is the only item in this pre-check that is.**
§3 calls C4b "declaration-moving … a new request form". A new request form on the existing
operation would be — but it does not have to be one. `rule versioning.compatible_change` names
**"adding an operation" first among the five**, and RFC 0026 has taken exactly that route twice
with a stated precedent: "'Adding an operation' is the first compatible change
`rule versioning.compatible_change` lists, so the minor is raised once at the end of the work and
the major window is untouched: a 3.0, 3.1, or 3.2 client is served exactly what its version
defines, and an operation it never names cannot reach it." F13's rule applies — the registry edit
lands in plan §10.2, RFC 0027's authority table and the IDL together.

> **Modified mechanism.** Do **not** touch `workspace.create` or `SnapshotComponents`. Declare a
> new operation that creates a snapshot from a registered corpus port named by its content
> commitment, plus whatever registration verb RFC 0019's manifest needs, and leave every existing
> declaration exactly where it is. `workspace.create` keeps all eleven members and every 3.x
> client keeps its answer.

**Priced by §1's method.** A by-reference request carries what a port cannot: the intent handle
and the snapshot epochs, 132 B measured, plus a port commitment at 77 B, plus braces — **211 B
against 718 B for Die Hard and 660 B for Philosophers**. Over the 24 recorded calls:
**17,208 − 5,064 = 12,144 B, 494 B per solved task.** Charging two port registrations at the full
recorded argument cost of one create each, 1,378 B, the net is **10,766 B, 438 B per solved
task**. Both figures are projections re-priced by the ledger's own method, not landed
measurements, and the registration charge is an upper bound: on this mix the daemon already holds
both ports.

**Correction this section carries forward.** §3 C4b's 12,888 B is superseded by the measured
12,144 B, and §3's warning stands — the landed `redesign_reducible` figure of 17,208 B is an
upper bound and not an available saving, because the intent handle and the snapshot epochs must
travel whatever the mechanism.

### 8.6 C6 — connection-scoped handle aliases — MAJOR-GATED, AND BLOCKED ABOVE THE VERSION QUESTION

**The decoder cannot help here, and that is the finding.** IDL §4 declares a handle as
`<prefix><opaque>` where the opaque part matches `[A-Za-z0-9_-]+`, and
`alias ArtifactHandle = String @pattern("^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$")` at IDL 952.
Measured: `TaskHandle::new("task_7")` **succeeds**, `ArtifactHandle::new("task_7")` **succeeds**,
and a real recorded answer with its task handle rewritten to `task_7` **decodes**, 189 B smaller
on an 819 B frame. So unlike C5, where a conforming reader refuses the frame, **no conforming
reader can refuse an alias** — an un-upgraded 3.5 client accepts it silently and stores a
session-scoped ordinal as an artifact identity. A minor whose failure mode is silent
misinterpretation by conforming clients is worse than one the decoder rejects.

**Two rules forbid it, and they are not version rules.**

```text
rule handshake.no_session_state {                       // IDL 2124
  The handshake establishes version, encoding, and authority only. No
  semantic state is session-scoped: a dropped connection MUST NOT change
  the meaning of anything (INV-002). Reconnecting with the same handles
  and a valid capability continues the work.
}
rule versioning.handle_stability {                      // IDL 708
  Handles remain valid across daemon upgrades within a protocol major.
}
```

An alias is a name whose meaning is session-scoped by construction, and it is not valid on a
reconnection. IDL §4's own header adds a third: "Handles carry a kind prefix and are otherwise
**structureless**" — an ordinal is structure. Rewriting `handshake.no_session_state` is not a
version question at all: it is INV-002, and it is the same constraint §3 C6 already recorded
against INV-004, "a certificate is checked from its wire form". **A major does not unblock this
item; an invariant decision does, and that is above the redesign.**

**And the INV-004 constraint costs most of the prize anyway.** §3 C6 restricts aliasing to
envelope-level references, never inside a `payload` that is stored, published or checked.
Measured over the matrix, splitting the 628 digest occurrences by whether they sit inside
`payload`: **envelope-level 336 occurrences at 21,504 B, payload-level 292 at 18,688 B.** Of the
envelope-level bytes, 88 occurrences at 5,632 B are first mentions and **248 at 15,872 B are
re-sends** — so the conforming ceiling is 15,872 B, **646 B per solved task**, before any overlap
with C1 and C5 is removed, and **46.5% of the digest bytes are out of reach by invariant.**

**C6 at 3.6: 0 B.** Nothing of it is available, and a negotiated feature identifier does not
rescue it: `ClientHello.features` and `ServerWelcome.features` would make both parties agree to
alias, and the aliased frame would still be a frame whose meaning dies with the connection.

### 8.7 Round-2 errata

**Carried from `bn-2in7i` and adopted here.**

1. **C5's bytes are CONFIRMED, not falsified — only its class was wrong.** 27,864 B of `epochs`
   exactly as §2.6 projected, plus 5,332 B of empty-member skeleton against §3 C5's 5,160 — a
   **172 B erratum in the ledger's favour**, the skeleton is 31 B per answer and not 30. Total
   **1,383 B per solved task** against the projected 1,344. C5 is worth more than C1, C2, C3, C4
   and C6 combined, and none of it is collectable before a major.
2. **The 8,084 B protocol-only `epochs` credit is UNCOLLECTABLE.** §2.6 reads it as an
   understatement of the 27,864 B prize by 3.4x, and the arithmetic is right — 27,864 / 8,084 =
   3.4 — but the reading is beside the point, because **the 8,084 B is not a saving anyone may
   take**. Reducing the set to `protocol` plus six explicit nulls moves no presence marker, and
   five of those six nulls would be **false**: this daemon pins `semantic`, `intent`, `proof`,
   `corpus` and `engine`, and only `evidence` is genuinely unpinned and already null.
   `rule envelope.epochs_named` gives null exactly one meaning — "an epoch the result cannot pin
   reads null; it is never an absent field" — so writing it over a pinnable epoch is a claim
   about the daemon, not an encoding choice, and RFC 0027's Safety section names that trade.
   Measured available-at-3.5 for the whole of C5: **0 B**.
3. **RFC 0027's correction-31 addendum headline is overstated by the two largest items in the
   ledger.** C7's 1,114 B per solved task and C5's 1,383 B are both off the 3.6 table — 2,497 B
   per solved task of the 47.6% headline. This note does not edit the RFC; a follow-up owns the
   addendum correction, and after this section it must also carry the four items below.

**New, from this pre-check.**

4. **§2.2's per-member `artifacts` table over-charges by 1 B per member instance.** Measured with
   the same decoder-strip method: `kind` is **1,896 B** and not 2,040; `commitment` is
   **7,140 B** and not 7,224. Both errata are in the ledger's disfavour and neither changes a
   verdict.
5. **§3 C2's projection is 1,800 B high.** The manifest collapse measures **10,656 B, 434 B per
   solved task**, against the projected 12,456 B and 506.
6. **§3 C4b's projection is 744 B high.** A by-reference request measures 211 B, so the saving is
   **12,144 B, 494 B per solved task**, against the projected 12,888 B and 524.
7. **§3 C1's premise is false on this deployment.** "A later answer about the same artifact …
   carries no information the client does not hold" fails on **8 of 92 repeat refs**, which carry
   a changed `commitment` under an unchanged `handle`. Restricted to the 84 that are genuinely
   byte-identical, move 3 is worth 12,440 B and not 16,524 B.

### 8.8 The sum-check, restated once more

The target is unchanged and is restated so no reader has to reconstruct it. The ratified margin
is +30% and the baseline spends 4,118 B per solved task, so a passing native arm must land at
**native ≤ 4,118 x 0.70 = 2,882 B per solved task**, which from today's 10,859 requires giving
back **7,977 B per solved task**.

| candidate set | saved over the matrix | saved / solved | native after | margin |
|---|---:|---:|---:|---:|
| landed measurement | — | — | 10,859 | **−163%** |
| §4's five commissioned items, as projected | 83,676 | 3,405 | 7,454 | −81% |
| §4's corrected six-item 3.6 set, C5 skeleton bundled | 104,940 | 4,270 | 6,589 | −60% |
| **the 3.6 bundle after this pre-check — C4b alone** | **12,144** | **494** | **10,365** | **−152%** |
| the same, charging two port registrations | 10,766 | 438 | 10,421 | −153% |
| everything, 3.6 plus the 4.0 horizon, best case | ~100,900 | ~4,107 | ~6,752 | −64% |
| **required for the ratified floor** | — | **7,977** | **≤ 2,882** | **+30%** |

Read three ways, and all three matter.

- **The 3.6 bundle recovers 494 B of the 7,977 B the floor needs — 6.2% of the gap** — and
  removes **4.5% of the native arm's total wire spend**, against §5 route 4's corrected 37.4% and
  its original 47.6%.
- **3,566 B per solved task, 87.8% of the corrected six-item set, moves to the 4.0 horizon.**
  With the C5 skeleton counted the figure is 3,776 B and 88.4%.
- **The verdict of §4 and §5 is unchanged and is now unreachable at two removes.** Even granting
  every item at a major — C5's 1,383, C1's 583, C2's 434, C3's 567, C4b's 494 and C6's 646 —
  the set reaches about 4,107 B per solved task and lands at −64%, which is §4's −65% within
  rounding. No redesign of this encoding clears the margin; §5's algebra is why, and it is
  untouched by anything in this section.

### 8.9 Recommendation

**What remains for a 3.6 bundle is one item, in a form the ledger did not specify, worth 6.2% of
the gap.** That is C4b as a **new operation** — `bn-2ug29`, §8.5 — and it is worth commissioning
on its own merits, because it is a real saving, it needs no declaration to move, and RFC 0026 has
twice taken the same route with a stated precedent. Two zero-byte companions may ride it if a
major is later intended: C2's and C3's `ServerWelcome` profiles as new `optional` members, which
are admissible today and collect nothing until the answer side follows them at 4.0.

**Everything else is a 4.0 question, and the lead should escalate it.** C1, C4a and C6 are
major-gated on their own declarations; C2's and C3's savings live entirely in the answer half
that a major must carry; C5 was already ruled a major by `bn-2in7i`; C7 is withdrawn. That is
**six of the seven ledger items on the far side of a protocol major**, which is a different
decision from the one the redesign was commissioned as. It is not a byte question any more:

- **A major must publish the plan §4.6 typed `Preserved | Revalidate | Incompatible` statement
  per artifact class before it is applied**, and the daemon serves majors N and N−1, so every
  suppressed member must still reach a 3.x client on the same daemon. `bn-2in7i`'s bone comment
  carries the seven-step activation checklist for C5; C1, C2, C3 and C4a each need their own.
- **C6 does not become available at 4.0.** It is blocked by INV-002 and INV-004, not by a version
  rule, and it needs an invariant decision that no redesign bone may take.
- **The exit sentence is still satisfied by the redesign having happened**, which is §5 route 4's
  reading and is unchanged. What this section removes is the claim that 37.4% of the typed arm's
  wire spend can be recovered inside major 3. The available figure inside major 3 is **4.5%**.

**Do not dispatch C1, C2, C3, C4a or C6 against the §3 or §6 tables.** Both stay above as the
record of what was projected, and both are superseded on wire visibility by §8.2. Each of the
five bones carries a comment naming its verdict and the declaration that blocks it.

---

## 9. C4b landed — the measurement, against §8's projection

**Amendment, `bn-3of5h`, 2026-08-09.** Nothing above this heading is rewritten. §8 stays as
the record of what was projected and at which version it was collectable; this section is
what the projection turned out to be worth once it was on the wire.

`workspace.create_by_reference` is the operation, at protocol **3.6** (IDL 1.11) — the 75th
row, the third the registry has ever gained, and the first to enter an existing namespace.
`rule snapshot.by_reference` is its semantics. The full three-source registry edit landed
together per F13, and the dossier validator's three-way parse enforces it rather than
remembering it.

### 9.1 The numbers

| | before | after | delta |
|---|---:|---:|---:|
| native, B per solved task | 10,859 | **10,384** | **−475** |
| native, B over the matrix | 260,630 | 249,230 | −11,400 |
| `workspace.create`, B per call (request + result) | 1,471 | 996 | −475 |
| **shell, B per solved task** | **4,118** | **4,118** | **0** |
| byte margin against the +30% floor | −163% | **−152%** | +11 pts |

**Against §8.5's projection of 494 B per solved task the landed figure is 475 — 96.2% of
it**, and §8.8's predicted "native after 10,365, margin −152%" lands at 10,384 and −152%.
The 19 B/solved shortfall is one erratum in the projection's favour and it is stated rather
than absorbed: §8.5 priced a by-reference request at 211 B from a 77 B port commitment plus
132 B of intent and epochs. The landed request is larger. A commitment on this wire is a
`ws_`-prefixed 64-hex token — 69 B with its quotes — and the model charged nothing for the
member keys (`"components"`, `"epochs"`, `"intent"`, `"seal"`) or for the `epochs` object's
own braces and keys. The saving per call is therefore 474 B where 506 was projected.

### 9.2 What did not move, and why that is the load-bearing half

**The baseline's numbers are byte-for-byte unchanged** — 4,118 B per solved task, 98,836 B
total, 69‰ invalid, 24/24 solved, and its `workspace.create` row still 24 calls at 9,000 B.
The anti-gaming discipline asks whether a disciplined shell user could mirror the typed
arm's change, and here the question runs the other way: **the baseline has named the port
since the instrument was built.** Its command line is
`continuum workspace create --port TV-009 --seal false --as agent:builder`, and the CLI
resolves the port's components from the filesystem. PR-10 IMPL-02 recorded the asymmetry in
as many words — "a local CLI names a corpus port and resolves its components from the
filesystem while a remote protocol client must transmit `SnapshotComponents` in full" — and
this operation is that sentence paid. There is nothing to mirror because the mirror already
exists; what changed is that the typed arm may now say the same thing.

Three further controls, each a test in
`crates/continuum-benchmark/tests/pr10_c4b_port_by_reference.rs`:

- **The protocol bump costs zero bytes.** `"3.2"` and `"3.6"` are the same length and
  `ProtocolVersion` is the only place either appears, so the handshake is the same 875 B and
  every envelope is the same size. The whole delta is attributable to the request argument.
- **Only one row of the per-operation table moved.** `task.status`, `task.resume`,
  `verification.start`, `verification.result`, `workspace.fork` and `workspace.seal` are
  their 3.5 byte totals exactly.
- **Both arms still attempt one sequence.** `Call::operation()` reports the *act*, and a
  by-reference create is `workspace.create` with a different argument, so the shared policy
  still decides one script and `both_arms_attempt_the_identical_operation_sequence` is
  unweakened. The same `ws_` handle comes back, so nothing was traded for the bytes.

`bn-2phq3`'s control family and `bn-2c0a`'s seventeen falsification tests are all green.
The control family's mechanism is untouched — it still compares the control run to the
landed run over whole `ArmRun` values, every counter and every transcript line — and only
the landed value it is compared *at* has moved. Every one of the thirteen attacks keeps its
verdict: eight LANDED, four HELD, one INCONCLUSIVE. What moved is the size of the loss and
not any conclusion about it.

### 9.3 The sum-check, once more, with the landed figure

| candidate set | saved / solved | native after | margin |
|---|---:|---:|---:|
| landed measurement, before this bone | — | 10,859 | −163% |
| §8's 3.6 bundle as projected — C4b alone | 494 | 10,365 | −152% |
| **§9's landed measurement — C4b alone** | **475** | **10,384** | **−152%** |
| required for the ratified floor | 7,977 | ≤ 2,882 | +30% |

**C4b recovers 5.9% of the 7,977 B the floor needs and removes 4.4% of the native arm's
total wire spend.** Both figures are within a tenth of a point of §8.8's projections (6.2%
and 4.5%), which is the useful result of this bone beyond the bytes: **the ledger's method
predicts a landed measurement to within 4%.** §8.9's recommendation stands unchanged and is
now the record of a completed item rather than a proposal: everything else is a 4.0
question, the exit sentence is still satisfied by the redesign having happened, and no
redesign of this encoding clears the margin.
