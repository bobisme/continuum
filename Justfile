# Continuum project tasks.
#
# `just check` and `just install` are the gates named in .edict.toml.

set positional-arguments

# List the available recipes.
default:
    @just --list

# Full project gate: formatting, lints, tests, crate boundaries, governance, dossier, Lean.
check: fmt-check lint test boundaries governance dossier lean

# Reject unformatted Rust.
fmt-check:
    cargo fmt --all --check

# Format the workspace in place.
fmt:
    cargo fmt --all

# Clippy over every target, warnings are errors.
#
# `--locked` (here and below) is the mechanical half of docs/12 §5, which lists
# "locked dependencies" among the reproducibility assets, and of plan §4.2, where a
# workspace snapshot pins "dependency lockfiles". Without it a gate silently resolves
# and rewrites `Cargo.lock`, so the run that passed is not the run anyone can repeat.
# Adding a dependency is therefore an explicit two-step: change `Cargo.toml`, run
# `cargo check` (unlocked) to refresh `Cargo.lock`, and commit both.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Workspace test suite.
test:
    cargo test --workspace --locked

# Build the workspace.
build:
    cargo build --workspace --locked

# Enforce the plan §20 crate list and the forbidden dependency edges.
# The self-test runs first so the check cannot pass vacuously.
boundaries:
    python3 tools/check_crate_boundaries.py --self-test
    python3 tools/check_crate_boundaries.py

# Enforce the docs/12 executable policy obligations (GOV §1 code/semantic
# policy, GOV §2 ADR process, GOV §3 claim governance). Each checker runs its
# self-test first — every violating fixture must be caught — so none of the
# three can pass vacuously. Evidence lands in tools/governance/evidence/.
governance:
    python3 tools/governance/check_code_policy.py --self-test
    python3 tools/governance/check_code_policy.py
    python3 tools/governance/check_adr_process.py --self-test
    python3 tools/governance/check_adr_process.py
    python3 tools/governance/check_claim_governance.py --self-test
    python3 tools/governance/check_claim_governance.py

# Mechanical validation of the architecture/research dossier.
dossier:
    cd notes/plan && uv run --with jsonschema python3 tools/validate_dossier.py

# Lean metatheory gate: build the RFC 0012 T0/T1 rungs and verify the ADR-0035
# axiom manifest against the checked-in artifacts (`lean/artifacts/`).
#
# Toolchain layering (mise.toml): mise pins the elan release, elan reads
# `lean/lean-toolchain` and installs/selects the Lean version named there. `lake`
# is an elan proxy, so it is looked up on PATH and no elan or toolchain location
# is hardcoded here — whichever install provides it (mise-exposed, or
# `~/.elan/bin` from `. ~/.elan/env`) is the one the gate uses.
#
# `axiom-manifest.sh --check` is read-only with respect to `lean/artifacts/`: it
# generates into a scratch directory and diffs, so a stale manifest fails the
# gate instead of being silently rewritten under it.
lean:
    @export PATH="$HOME/.elan/bin:$PATH"; \
    command -v lake >/dev/null 2>&1 || { \
        echo "continuum: 'lake' is not on PATH (also tried ~/.elan/bin)." >&2; \
        echo "  Lean comes from elan: mise.toml pins the elan release and" >&2; \
        echo "  lean/lean-toolchain pins the Lean version elan installs." >&2; \
        echo "  Bootstrap: run 'mise install' (this installs elan-init), then" >&2; \
        echo "  'elan-init -y'." >&2; \
        exit 1; \
    }; \
    (cd lean && lake build) && sh lean/scripts/axiom-manifest.sh --check

# Regenerate the plan/Bones traceability registry and report.
traceability:
    cd notes/plan && uv run --with jsonschema python3 tools/generate_traceability.py

# Install Continuum binaries.
#
# Honest as of PR-1 / IMPL-01: no crate declares a [[bin]] target yet. Plan §20
# names `continuumd` (PR 5) and `continuum-cli` (PR 13) as the future binaries;
# replace the message below with `cargo install --path crates/<name> --locked`
# when either one gains an entry point.
install:
    @echo "continuum: nothing to install yet — no crate declares a [[bin]] target."
    @echo "           plan §20 names continuumd (PR 5) and continuum-cli (PR 13)"
    @echo "           as the first binaries; fill in this recipe when one lands."
