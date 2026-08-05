# Continuum project tasks.
#
# `just check` and `just install` are the gates named in .edict.toml.

set positional-arguments

# List the available recipes.
default:
    @just --list

# Full project gate: formatting, lints, tests, crate boundaries, kernel covenant,
# governance, dossier, Lean.
check: fmt-check lint test boundaries covenant governance dossier lean

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

# Enforce the docs/03 §5 / plan §20 kernel covenant over the four
# `continuum-kernel-*` crates: the <15,000 non-test-line budget (with the counting
# method documented in the checker), the no-async/no-unsafe/no-panic lint walls,
# dependency freedom, the mutation-class matrix, and INV-014 receipt conformance.
# The self-test runs first — every violating fixture must be caught — so the check
# cannot pass vacuously. Evidence lands in tools/kernel-covenant/evidence/.
#
# `--strict` (not run here) promotes the matrix's `uncovered` waivers to failures;
# it is the burn-down switch, not the gate.
covenant:
    python3 tools/check_kernel_covenant.py --self-test
    python3 tools/check_kernel_covenant.py

# Enforce the docs/12 executable policy obligations (GOV §1 code/semantic
# policy, GOV §2 ADR process, GOV §3 claim governance), the docs/09 T12
# dependency vet/advisory-scanning posture, the T12 six-control binding
# over all of the above, the docs/08 R16 five-control binding (Rust
# compiler churn: pinned toolchain, narrow compiler-internal usage,
# extraction adapters isolated, compatibility CI, prefer stable metadata
# over rustc internals), and the docs/09 T04 six-control binding (forged
# trace or artifact substitution: content-addressed manifests, build
# identity, reject digest mismatch, and preserve redaction commitments
# bound to live enforcement; hash chain/Merkle root over events and
# optional signing/attestation recorded as typed absences, each with its
# own mechanical absence-check). Each checker runs its self-test first —
# every violating fixture must be caught — so none of these can pass
# vacuously. Evidence lands in tools/governance/evidence/.
#
# `check_revision_delta.py` is the two-revision half of GOV-1-08/09: the other
# six checkers read one revision, and "this change was semantic" / "this change
# was breaking" are not one-revision properties. It runs *last* and guards
# itself on a resolvable base rather than being guarded here, because a shell
# guard would have to duplicate the base-resolution logic and could then
# disagree with it: with no merge base — a fresh clone, a shallow checkout, an
# unrelated history — the checker prints why and skips, exit 0, and says in its
# report that the delta rules were not enforced. CI adds `--require-base`, which
# turns that same skip into a failure where a base is genuinely owed:
#
#     python3 tools/governance/check_revision_delta.py --base origin/main --require-base
governance:
    python3 tools/governance/check_code_policy.py --self-test
    python3 tools/governance/check_code_policy.py
    python3 tools/governance/check_adr_process.py --self-test
    python3 tools/governance/check_adr_process.py
    python3 tools/governance/check_claim_governance.py --self-test
    python3 tools/governance/check_claim_governance.py
    python3 tools/governance/check_dependency_audit.py --self-test
    python3 tools/governance/check_dependency_audit.py
    python3 tools/governance/check_t12_evidence.py --self-test
    python3 tools/governance/check_t12_evidence.py
    python3 tools/governance/check_r16_evidence.py --self-test
    python3 tools/governance/check_r16_evidence.py
    python3 tools/governance/check_t04_evidence.py --self-test
    python3 tools/governance/check_t04_evidence.py
    python3 tools/governance/check_revision_delta.py --self-test
    python3 tools/governance/check_revision_delta.py

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
#
# It then asserts the manifest is *empty*. The two are different obligations and
# the diff does not imply the assertion: ADR-0035 requires axioms to be
# documented, which the diff enforces, while START_HERE's PR-4a exit and plan §21
# Phase A require T0/T1 to compile "with no `sorry` and empty axiom manifests",
# which the diff cannot see — a `sorry` landed together with a regenerated
# manifest documents its own `sorryAx` and diffs clean. The script's header states
# the reading in full (bn-bou09).
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

# ASan/TSan lanes over the daemon and publication paths (PHASE-A-DEL-03, bn-cho5).
#
# Deliberately NOT part of `just check`: `-Zsanitizer` is nightly-only, and the
# workspace toolchain is pinned to 1.97.0 (rust-toolchain.toml, INV-014) — the
# default gate must stay green on the pinned toolchain. This lane needs a nightly
# toolchain with the `rust-src` component (`-Zbuild-std` instruments std itself,
# without which TSan reports false positives against uninstrumented std sync
# primitives): `rustup toolchain install nightly && rustup component add
# rust-src --toolchain nightly`. Pass a dated nightly to pin the lane's identity:
# `just sanitizers nightly-2026-04-29`.
#
# What the lane is, and is not: sanitizer output is *testing evidence, never
# proof* (docs/29 §"assurance stack", research/20) — the deterministic schedule
# matrices (`task_lifecycle_schedule_matrix.rs`, `publication_schedule_matrix.rs`,
# the DX-13/DX-14 campaigns) carry the semantic claims; this lane checks the
# memory-model residual those single-threaded enumerations cannot reach: that the
# store's `Mutex` really does make its critical sections atomic under real
# threads (the `continuum-workspace` threaded suites), and that the daemon and
# publication paths are free of UB the pinned-toolchain gate cannot see.
#
# Anti-vacuity: each sanitizer first proves it is ARMED by compiling and running
# a canary with a known defect (a heap overflow for ASan, a data race for TSan)
# and requiring a non-zero exit — a mis-spelled RUSTFLAGS would otherwise turn
# the whole lane into a green no-op. Doctests are excluded (`--tests`): they run
# under the pinned gate, and rustdoc's build-std interaction is not this lane's
# claim.
sanitizers toolchain="nightly":
    #!/usr/bin/env bash
    set -euo pipefail
    if ! rustup run "{{toolchain}}" rustc --version >/dev/null 2>&1; then
        echo "continuum: toolchain '{{toolchain}}' is not installed." >&2
        echo "  rustup toolchain install {{toolchain}}" >&2
        exit 1
    fi
    if ! rustup component list --toolchain "{{toolchain}}" 2>/dev/null | grep -q '^rust-src.*(installed)'; then
        echo "continuum: '{{toolchain}}' lacks rust-src (needed by -Zbuild-std)." >&2
        echo "  rustup component add rust-src --toolchain {{toolchain}}" >&2
        exit 1
    fi
    host="$(rustup run "{{toolchain}}" rustc -vV | awk '/^host:/ {print $2}')"
    echo "== sanitizer lane: {{toolchain}} ($(rustup run "{{toolchain}}" rustc --version)), target ${host}"
    canary_dir="$(mktemp -d)"
    trap 'rm -rf "${canary_dir}"' EXIT
    printf '%s\n' \
        'fn main() {' \
        '    let v = vec![0u8; 4];' \
        '    let p = v.as_ptr();' \
        '    unsafe { std::ptr::read_volatile(p.add(4)); }' \
        '}' > "${canary_dir}/asan_canary.rs"
    printf '%s\n' \
        'static mut RACER: u64 = 0;' \
        'fn main() {' \
        '    let t = std::thread::spawn(|| unsafe { RACER += 1 });' \
        '    unsafe { RACER += 1 };' \
        '    t.join().unwrap();' \
        '}' > "${canary_dir}/tsan_canary.rs"
    for lane in address thread; do
        case "${lane}" in
            address) canary="asan_canary" ;;
            thread)  canary="tsan_canary" ;;
        esac
        echo "== ${lane}: arming check (the canary's defect must be caught)"
        # `-Cunsafe-allow-abi-mismatch=sanitizer` is sound for the CANARY only: its
        # defect lives entirely in its own (instrumented) crate, and the waiver merely
        # lets it link the prebuilt std without a per-canary `-Zbuild-std`. The suites
        # below never use the waiver — they get a fully instrumented std.
        rustup run "{{toolchain}}" rustc --edition 2021 -Zsanitizer="${lane}" \
            -Cunsafe-allow-abi-mismatch=sanitizer \
            --target "${host}" -o "${canary_dir}/${canary}" "${canary_dir}/${canary}.rs" \
            2> "${canary_dir}/${canary}.compile.log" \
            || { cat "${canary_dir}/${canary}.compile.log" >&2; exit 1; }
        if "${canary_dir}/${canary}" > "${canary_dir}/${canary}.log" 2>&1; then
            echo "continuum: the ${lane} sanitizer did NOT catch its canary — the lane is disarmed." >&2
            exit 1
        fi
        echo "== ${lane}: armed; running the daemon and publication suites"
        export RUSTFLAGS="-Zsanitizer=${lane}"
        cargo +{{toolchain}} test --locked -Zbuild-std --target "${host}" --tests \
            -p continuum-workspace -p continuumd -p continuum-task
        unset RUSTFLAGS
    done
    echo "== sanitizer lane: both lanes green (evidence, never proof — docs/29)"

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
