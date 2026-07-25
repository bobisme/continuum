# RFC 0023: Checked Choreographic Projection

## Summary

Project CML global protocols to local role automata and generated Rust interfaces.

## Outputs

- local role state machines;
- message enums and typed payloads;
- endpoint traits;
- asupersync task skeletons;
- instrumentation labels;
- local monitors;
- projection receipt.

## Rejection conditions

Projection fails when a role must choose without knowing the branch, communication assumptions mismatch, cancellation/fault branches are missing, or local composition admits behavior outside the global model.

## Non-goal

Generated code does not prove arbitrary user implementation code. It provides a checked interface and refinement target.
