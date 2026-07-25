# Research Note 09: Cancellation and Obligation Calculus

**Claim class:** core research program  
**Foundation:** asupersync lifecycle semantics

## Motivation

Most formal models treat cancellation as task disappearance or a Boolean flag. Real async software can be canceled between reservation and commitment, while holding capacity, locks, reply duties, durable-write intents, or child tasks. This is a major source of bugs and an under-modeled semantic dimension.

Asupersync makes cancellation explicit: request → drain → finalize, with region ownership and obligations. Continuum should turn that engineering discipline into a formal calculus.

## Core objects

Let:

- \(R\) be a tree of regions;
- \(T\) tasks owned by regions;
- \(O\) linear obligations;
- \(E\) effect instances with phases;
- \(B\) cleanup budgets/capabilities.

State tracks:

```text
owner(o) ∈ Task ∪ Region ∪ External
phase(o)
discharge condition
transfer history
deadline/budget where applicable
```

Obligations cannot be duplicated. Dropping requires an explicit abort/discharge transition.

## Effect protocol

A two-phase effect:

\[
Idle \to Reserved(o) \to Committed
\]
or
\[
Reserved(o) \to Aborted.
\]

Cancellation request prevents new ordinary work under policy, initiates drains, and eventually requires every owned obligation to be discharged/transferred or explicitly classified as external/non-cooperative.

## Safety invariants

- no orphan tasks;
- region closure implies no live descendants;
- every obligation has exactly one owner;
- committed effects are not aborted;
- reserved effects do not vanish;
- finalization runs at most once;
- reply/ack obligations are not duplicated;
- epoch changes invalidate stale capabilities.

## Progress

Under cooperative polling and finite cleanup creation:

- cancellation request eventually reaches terminal outcome;
- drain phase does not create unbounded obligation mass;
- region tree reaches quiescence.

Potential ranking:

\[
(\text{region phase},
 \mathsf{multiset}(\text{obligation phase/depth}),
 \#\text{live tasks},
 \text{budget})
\]

ordered lexicographically/multiset-wise.

## Static surface

Possible annotation:

```rust
#[continuum::effect]
async fn send_reply(
    cx: &Cx,
    permit: ReplyPermit,
    value: Reply,
) -> Outcome<(), Error>
where
    permit: consumes ReplyObligation;
```

The initial implementation should infer events from asupersync primitives and use runtime checking. A future static checker/Verus integration can prove obligation conservation locally.

## Capability security

`Cx` and tokens express authority. Continuum records capability creation, delegation, use, and revocation as resource events. This supports:

- least-authority audits;
- cross-task ownership;
- cancellation-safe delegation;
- security views where unauthorized effects violate refinement.

## Foreign calls

An FFI/blocking operation may be non-cooperative. The model must choose one:

- bounded, cancel-aware adapter;
- isolated external obligation with watchdog/recovery;
- process-level kill;
- unsupported path.

No universal cancellation guarantee is inferred.

## Novel proposal: Session-Typed Cancellation

Treat operation lifecycle as a local session:

```text
request . (commit . end ⊕ cancel . drain . finalize . end)
```

Global cancellation across task trees becomes a choreography. Projection can generate local protocol states and monitor transitions. This may prevent mismatched parent/child shutdown logic.

## Mutation corpus

- cancel between reserve and commit;
- winner returns before loser drains;
- child spawned during region close;
- reply obligation lost on panic;
- finalizer performs unbounded retry;
- stable write acknowledged before sync;
- obligation transferred to crashed epoch;
- timeout implemented as silent future drop;
- external call never returns;
- cancellation reason overwritten.

## Deliverables

1. Formal small-step calculus.
2. CIR mapping.
3. executable reference model.
4. invariant and liveness properties.
5. asupersync differential corpus.
6. drain ranking certificate.
7. optional session/choreography prototype.
8. paper-quality semantics if the theory proves genuinely novel.
