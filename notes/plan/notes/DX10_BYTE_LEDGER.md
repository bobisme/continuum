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
| result-frame bytes | 170,584 | `dx10_falsification.rs::s6_*` |
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
accepted, keyed into the idempotency state, and ignored.

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

### C7 — `task.status`: honor `OutputPolicy`, summarize with an expansion route

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

### The round-trip lever, tested and rejected

The obvious remaining move is fewer exchanges rather than smaller ones, and the protocol
already supports it: `verification.await` is declared in `continuumd::protocol::registry`
(`@readonly @task_starting`, response `VerificationResult`) and implemented in the daemon,
and `workspace.create` already takes `seal`. Collapsing `start` + `poll*` + `result` into one
`await`, and `create` + `seal` into one call, takes the matrix from 172 dispatches to 60.

Applied to the typed arm alone and combined with every candidate above, that lands at 3,072 B
per solved task — a +25% margin, which *looks* like it nearly clears the floor.

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
