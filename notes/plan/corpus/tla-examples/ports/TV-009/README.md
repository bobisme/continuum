# TV-009 — Die Hard

First corpus fixture. The executable Python reference port is in `spikes/models.py`; this directory records intended CML and model configuration.

Frozen spike facts:

- 16 reachable states;
- 96 labeled successor edges, including stuttering-producing actions;
- shortest state with `big = 4` at depth 6;
- closure/type certificate accepted by the independent Python checker.

P4 remains blocked until the pinned Lean source builds and the proof receipt is produced.
