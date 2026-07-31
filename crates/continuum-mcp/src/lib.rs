//! `continuum-mcp` — the MCP agent adapter (plan §10.4, §17.4, PR 27).
//!
//! # Responsibility
//!
//! A curated projection of native operations for agents: explicit handles,
//! deterministic schemas and ordering, result bounds, and capability checks.
//!
//! Two subagents may share one workspace and intent while using isolated debugger and
//! proof handles, with no session coupling.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20);
//!   MCP exposes `continuumd` operations and adds no authority of its own.
//! - INV-015 — agent least authority: the adapter may only narrow capabilities, never
//!   widen them.
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
