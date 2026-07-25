# Choreographies, Multiparty Session Types, and Verified Projection

## Question

Can Continuum derive useful Rust interfaces and runtime monitors from a global protocol model without pretending generated code proves the implementation correct?

## Relevant ideas

A choreography describes communication globally. Projection derives one local behavior per role. Multiparty session types and communicating automata study when these projections are compatible, deadlock-free, and faithful to the global description.

This is directly relevant to Continuum because CML already names roles, messages, actions, causality, and fairness. The same declarations can generate:

- Rust message enums and codecs;
- typed role/endpoint APIs;
- asupersync task skeletons;
- instrumentation event IDs;
- local runtime monitors;
- model-based scenario generators;
- refinement obligations connecting generated interfaces to the global model.

## Proposed architecture

```text
CML global model
   ↓ checked projection
Role automata / local contracts
   ├── Rust types and endpoint traits
   ├── asupersync skeletons
   ├── production monitors
   └── refinement obligations
```

The generated artifacts constrain the implementation surface but do not dictate internal architecture. A role implementation may batch, pipeline, retry, persist, or use auxiliary tasks provided its visible behavior refines the role contract.

## Projection obligations

A successful projection receipt should establish:

1. every global communication has compatible send/receive projections;
2. local choices are known to the role that must make them, or are communicated before divergence;
3. channel assumptions match the selected domain pack;
4. projection does not introduce deadlock absent from the global model;
5. local traces compose into global traces modulo hidden/stuttering actions;
6. cancellation and crash branches are represented explicitly;
7. monitor verdicts are sound for the observation contract.

## Rust API sketch

```rust
#[continuum::role("Leader")]
trait LeaderEndpoint {
    async fn send_append(
        &mut self,
        cx: &Cx,
        follower: NodeId,
        msg: AppendEntries,
    ) -> Outcome<AppendReply, RpcError>;
}
```

The generated trait identifies semantic effects and event labels. The implementation remains ordinary Rust.

## Why this is not code generation theater

The value is not boilerplate reduction alone. Generated interfaces create a stable seam where:

- the model and implementation share names and types;
- instrumentation is complete by construction for covered operations;
- domain-pack assumptions are visible;
- agents receive precise missing-case diagnostics;
- refinement can reason over an explicit local contract rather than arbitrary code.

## Research experiments

1. Project a two-phase commit model to coordinator/participant role automata.
2. Generate asupersync endpoint traits and a monitor.
3. Implement a correct and mutant coordinator.
4. Check local monitor behavior and global refinement.
5. Compare developer friction with handwritten message plumbing.

## Promotion criterion

Promote when projection catches real interface drift, generated APIs remain idiomatic, and a proof or translation validator establishes trace preservation. Kill or narrow the feature if projection forces unnatural code structure or if generated monitors cannot handle operational refinements.

## Primary references

- sound/complete projection and projection failure diagnostics: [S109];
- generalized asynchronous projection: [S110];
- mechanized choreographic programming and Pirouette: [S111]–[S112];
- multiparty asynchronous session types and event-structure semantics: [S113]–[S114].
