# Continuum project tasks.
#
# `just check` and `just install` are the gates named in .edict.toml.

set positional-arguments

# List the available recipes.
default:
    @just --list

# Full project gate: formatting, lints, tests, crate boundaries, dossier.
check: fmt-check lint test boundaries dossier

# Reject unformatted Rust.
fmt-check:
    cargo fmt --all --check

# Format the workspace in place.
fmt:
    cargo fmt --all

# Clippy over every target, warnings are errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Workspace test suite.
test:
    cargo test --workspace

# Build the workspace.
build:
    cargo build --workspace

# Enforce the plan §20 crate list and the forbidden dependency edges.
# The self-test runs first so the check cannot pass vacuously.
boundaries:
    python3 tools/check_crate_boundaries.py --self-test
    python3 tools/check_crate_boundaries.py

# Mechanical validation of the architecture/research dossier.
dossier:
    cd notes/plan && uv run --with jsonschema python3 tools/validate_dossier.py

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
