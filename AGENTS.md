# AGENTS.md

This file provides guidance to coding agents (Claude Code, Codex, etc.) when working with code in this directory.

## Sibling repositories

This is the core monorepo for the IOTA node, Move framework, and CLI (Rust + Move). Related code lives in two sibling repos:

- [iota-rust-sdk](https://github.com/iotaledger/iota-rust-sdk) — **canonical source of types shared between clients/SDKs and the node** (crates `iota-sdk-types`, `iota-sdk-crypto`, `iota-sdk-transaction-builder`, …), the public Rust SDK, and its FFI bindings (Go, Kotlin, Python, C#, Swift). The SDK crates also compile to `wasm32`.
- [ts-packages](https://github.com/iotaledger/ts-packages) — TypeScript/JavaScript SDK packages (`@iota/iota-sdk`, `dapp-kit`, `kiosk`, …), wallet, explorer, wallet-dashboard and other apps.

### Where types live

When adding or moving a type, pick the **narrowest** scope that fits:

1. **Client-visible (used by both clients/SDK and the node)** → define in `iota-rust-sdk` and import here.
2. **Internal but shared across multiple crates in this repo** → define in `crates/iota-types/`.
3. **Used by a single crate only** → define inside that crate, not in `iota-types`.

`crates/iota-types/` historically accumulated many types; the direction of travel is to push them outward — client-visible ones into `iota-rust-sdk`, single-crate ones into their owning crate — and keep `iota-types` for genuinely cross-crate internal types only. Don't add a new type to `iota-types` unless it really has more than one consumer in this repo.

## Essential Development Commands

The Rust toolchain is pinned in `rust-toolchain.toml`.

### Building

```sh
# Build a specific crate. Generally don't need a release build.
cargo build -p iota-core

# Check code without building (preferred for fast feedback)
cargo check -p iota-core
```

### Linting and Formatting

```sh
# Rust lint (matches CI)
cargo ci-clippy

# Rust formatting (requires nightly rustfmt)
cargo +nightly fmt

# TOML / Markdown / YAML formatting
dprint fmt
```

Move-related linting for crates in `external-crates/move/` is run from inside the corresponding crate directory.

## Testing

```sh
cargo simtest -p iota-e2e-tests              # e2e / sim tests
IOTA_SKIP_SIMTESTS=1 cargo nextest run       # unit tests (skips #[sim_test])
```

- Always run tests before submitting.
- Tests are slow — set 10+ min timeouts. Narrow with `-p iota-types -p iota-core` (repeat `-p`) and `--lib` to skip integration tests.
- `cargo simtest` lives at `scripts/simtest/cargo-simtest`; install once with `scripts/simtest/install.sh`. The installed wrapper uses `git rev-parse --show-toplevel`, so it only works from inside this repo.

### Snapshot tests

Snapshots use [`cargo insta`](https://insta.rs/). When a test fails because a snapshot changed:

```sh
cargo install cargo-insta            # one-time
cargo insta review                   # interactively accept/reject pending snapshots
cargo insta accept                   # accept all pending snapshots without prompting
```

Framework changes additionally require a full snapshot refresh via `scripts/update_all_snapshots.sh`. Always inspect the diff before accepting — an unintended snapshot change usually means a real regression.

### Test attributes

- `#[test]` — sync unit tests.
- `#[tokio::test]` — async unit tests on real tokio. Default for anything async that doesn't need multi-node / network / time control.
- `#[sim_test]` (from `iota_macros`, defined in [crates/iota-proc-macros/src/lib.rs](crates/iota-proc-macros/src/lib.rs)) — multi-node / consensus / networking tests that need the deterministic simulator (`iota-simulator`: patched tokio, mocked network, simulated time). Used heavily in `iota-e2e-tests` and consensus tests.

`#[sim_test]` is dual-mode: under `cargo simtest` (with `--cfg msim`) it runs in `iota-simulator` with node-leak detection; under plain `cargo test` / `cargo nextest` it falls back to `#[tokio::test]` — and is skipped entirely when `IOTA_SKIP_SIMTESTS=1`. The fallback has no determinism and no simulated network/time, so don't rely on it. Only mark a test `#[sim_test]` if you actually need the simulator; otherwise use `#[tokio::test]` so it runs everywhere.

## High-Level Architecture

### Repository layout

```
iota/
├── crates/                     # ~106 Rust crates (most-touched ones listed below)
│   ├── iota/                   # CLI binary
│   ├── iota-node/              # Validator / fullnode binary
│   ├── iota-core/              # Core blockchain logic
│   ├── iota-types/             # Node-internal types (client-visible types live in iota-rust-sdk)
│   ├── iota-framework/         # Move system packages & on-chain stdlib
│   ├── iota-protocol-config/   # Protocol parameters & feature flags
│   ├── iota-json-rpc/          # JSON-RPC API server
│   ├── iota-graphql-rpc/       # GraphQL API server
│   ├── iota-indexer/           # Blockchain data indexer (Postgres-backed)
│   ├── iota-grpc-server/       # gRPC API server
│   ├── starfish/               # Consensus protocol (config, core, simtests)
│   └── …
├── iota-execution/             # Move execution layer
│   ├── cut/                    # Tool for cutting new execution-layer versions
│   └── latest/                 # iota-adapter / iota-move-natives / iota-verifier
├── external-crates/move/       # Move language compiler, VM, and tooling (separate workspace)
└── scripts/                    # Build, lint, simtest, release helpers
```

`external-crates/move/` is a separate Cargo workspace — `cargo build` from the repo root does not touch it. To work on the Move compiler/VM, `cd external-crates/move/` and run cargo from there.

### Key architectural patterns

1. **Validator / Authority system**: a set of validators processes transactions in parallel; each maintains its own state and participates in Byzantine consensus via Starfish.

2. **Object model**: object-centric (not account-centric). Each object has a unique ID and version, and is owned, shared, or immutable.

3. **Transaction flow**:
   - Client → transaction driver → authority client → validator.
   - Transactions touching only owned objects can start execution before consensus.
   - Transactions touching shared objects require consensus ordering before execution.

4. **Storage layer**: RocksDB for persistent storage, separate stores for objects / transactions / effects, checkpointing for state sync.

5. **Execution pipeline**: validation → certificate creation → execution → effects commitment. The Move VM executes smart contracts with gas metering. Non-conflicting transactions execute in parallel.

### Critical development notes

1. **Protocol config changes**: modifications to `crates/iota-protocol-config/src/lib.rs` can break network consensus. Re-check feature-flag gating, version bumps, and snapshots carefully before merging.
2. **NEVER disable or skip tests** — all tests must pass and stay enabled.
3. **NEVER use `#[allow(dead_code)]`, `#[allow(unused)]`, or other lint suppressions** to silence warnings — fix the underlying issue.

### Conventions reference

- [`RUST_CONVENTIONS.md`](RUST_CONVENTIONS.md) — full list of Rust style and safety rules (panics, error handling, naming, module layout, etc.).
- [`REVIEW.md`](REVIEW.md) — review depth tiers, cross-cutting checks (license headers, breaking changes, dependency hygiene), and language-specific review guidance.

## Comment Style

These rules apply to every comment you write or modify. They exist because comments authored from chat context tend to be verbose, restate the visible code, and embed conversational history that becomes stale once the PR is merged.

### Function- and item-level doc comments (`///`, `//!`, `/** */`)

Write them for the **caller**, not the implementer. The doc should answer "what do I need to know to call this correctly?" — nothing more.

**Include:**

- One short sentence describing what the function or type does, from the caller's point of view.
- Invariants the caller must uphold (preconditions on arguments or state).
- Surprising postconditions, side effects, or ordering guarantees.
- `# Errors`, `# Panics`, `# Safety` sections (Rust) where they apply — see [`RUST_CONVENTIONS.md`](RUST_CONVENTIONS.md).

**Do not include:**

- A step-by-step list of what the function does internally — that's the body.
- Rationale for design choices, rejected alternatives, or trade-offs considered.
- References to the change history: "added for X flow", "used by Y", "as discussed", PR or issue numbers.
- Restatements of what well-named parameters or return types already convey.

### Inline comments inside a function body

Default to **none**. A line of code with a good name does not need a comment explaining what it does.

Add a comment **only** when:

- There is a non-obvious _why_: a workaround for a known bug, a constraint imposed by code elsewhere, a deliberate departure from the obvious approach.
- An invariant holds at that exact point that a future reader would otherwise have to re-derive.
- The behavior at that line would surprise a careful reader.

Implementation details belong **next to the implementation**, not in the function header. If a fact only matters at one specific line, comment that line — do not lift the explanation into the docstring.

### Anti-patterns

Never write these, even if asked to "document thoroughly":

- "This function was added to handle the X case discussed with the team."
- "We chose approach A over B because B would have required ..."
- "TODO: revisit per the conversation on YYYY-MM-DD."
- Multi-paragraph rationale blocks attached to a function signature.
- Section headers (`# Implementation`, `# Notes`, `# Design`) that introduce a content dump rather than a documented contract.
- A comment immediately before `let x = foo();` that says "Get foo and assign it to x."

If rationale needs to live somewhere, it lives in the PR description, the commit message, or a design doc — never in a doc comment that future readers must scroll past every time they look up the function's contract.

### Worked example

Bad:

```rust
/// Computes the next epoch's committee.
///
/// This function was added as part of the consensus rework discussed in #1234.
/// It first loads the validator set, then filters out validators that have been
/// slashed, then sorts them by stake, then picks the top N. We chose to sort
/// by stake rather than by VRF output because the team agreed that stake-based
/// selection is more predictable for testing.
pub fn next_committee(epoch: Epoch) -> Committee { ... }
```

Good:

```rust
/// Returns the committee that will be active in `epoch`.
///
/// `epoch` must be the current epoch or the immediate next one; older or
/// further-future epochs return `Err(EpochOutOfRange)`.
pub fn next_committee(epoch: Epoch) -> Result<Committee, EpochError> { ... }
```

If the stake-vs-VRF choice matters for a reader of the body, the comment goes inside the function at the sort step — not in the header.
