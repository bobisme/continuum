# ADR-0028: Treat environment and fairness assumption synthesis as games

**Status:** Accepted as strategic research direction  
**Date:** 2026-07-24

## Context

Liveness failures are often dismissed by adding fairness assumptions until the model passes. This is dangerous: the assumptions may be stronger than production can provide, and the model checker gives little help identifying the weakest adequate contract.

Reactive synthesis and game theory provide a more honest formulation. The implementation and environment/fault scheduler are players. A property is realizable only under some environment strategy constraints.

## Decision

Continuum models distinguish controlled system choices, admissible implementation nondeterminism, stochastic choices, and adversarial environment/fault choices.

For supported finite omega-regular fragments, Continuum can:

- solve the corresponding safety/parity/Streett game;
- emit an environment counterstrategy when the property is unrealizable;
- synthesize candidate safety restrictions and fairness assumptions;
- order assumptions by a declared strength relation;
- validate generated assumptions against production/domain-pack capabilities;
- expose assumptions as versioned contracts, never hidden solver artifacts.

“Weakest” is used only relative to a specified assumption grammar/order; global weakest assumptions may not exist or be tractable.

## Consequences

The tool can answer a more useful question than “liveness failed”: what must the network, scheduler, storage, operator, or clients guarantee, and can that guarantee be implemented?

This also creates a foundation for agent-assisted protocol repair using counterstrategies rather than random patches.
