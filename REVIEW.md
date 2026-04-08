# REVIEW.md

This file instructs Claude Code on how to conduct code reviews in the IOTA monorepo. Read it fully before reviewing any pull request.

---

## Access control — check this FIRST

Before doing any review work, determine whether you are authorized to proceed. Only the following people are authorized to trigger a full Claude Code review:

- Alexander Sporn (`alexsporn`)
- Levente Pap (`lzpap`)
- Piotr Macek (`piotrm50`)
- Maximilian Hase (`muXxer`)
- Mirko Zichichi (`miker83z`)
- Thibault Martinez (`thibault-martinez`)
- Thoralf Müller (`Thoralf-M`)
- Konstantinos Demartinos (`kodemartin`)
- Luigi Vigneri (`vekkiokonio`)
- Thomas Shufps (`shufps`)
- Begoña Álvarez de la Cruz (`begonaalvarezd`)
- Lukas Möller (`lmoe`)
- Luca Moser (`luca-moser`)

This check applies to **every** review invocation — both the initial manual trigger and any subsequent automatic reviews on new pushes to the same PR.

**How to decide:**

1. **If this is a manual review trigger:** Check whether the person who requested the review is in the list above. If yes, proceed with the full review. If no, short-circuit (see below).
2. **If this is an automatic review on a subsequent push:** Check the PR's review history to see whether any whitelisted person has previously manually triggered a Claude Code review on this PR. If yes, proceed with the full review. If no whitelisted person has ever manually triggered a review on this PR, short-circuit (see below).

**Short-circuit behavior:** Do not perform the review. Instead, post a single comment stating:

> This review was not executed. Only users whitelisted in `REVIEW.md` can trigger a manual Claude Code review.

Then stop — do not continue with any of the sections below.

---

`RUST_CONVENTIONS.md` is the canonical source for Rust coding conventions. This file adds **review-specific guidance** across all languages (Rust, TypeScript, Move) and defines review depth, cross-cutting checks, and output format. Do not duplicate rules already in `RUST_CONVENTIONS.md` — reference it instead.

---

## Review depth by path

Apply scrutiny proportional to the blast radius of the code being changed.

**Tier 1 — Critical (consensus, execution, system contracts):**

- `consensus/core/`, `consensus/config/`
- `iota-execution/`
- `crates/iota-framework/packages/iota-system/`

Bugs here can cause network faults, forks, or asset loss. Every change is load-bearing until proven otherwise. Missing tests, unjustified panics, or unclear error handling are blocking.

**Tier 2 — High (protocol engine, types, Move VM):**

- `crates/iota-core/`, `crates/iota-types/`, `crates/iota-protocol-config/`
- `crates/iota-transaction-checks/`, `crates/iota-node/`, `crates/iota-network*/`
- `external-crates/move/`
- `crates/iota-framework/packages/iota-framework/`

Core protocol types and the Move language implementation. API stability, correct error propagation, and thorough tests matter here because many other crates depend on them.

**Tier 3 — Standard (RPC, indexing, storage, Rust SDK):**

- `crates/iota-json-rpc*/`, `crates/iota-graphql-rpc*/`, `crates/iota-rest-api/`, `crates/iota-grpc-*/`
- `crates/iota-indexer*/`, `crates/iota-analytics-indexer/`, `crates/iota-storage/`, `crates/typed-store*/`
- `crates/iota-sdk/`

Bugs affect users but not consensus. Review for correctness, error handling, and API consistency.

**Tier 4 — Moderate (TypeScript SDK, apps, dApps):**

- `sdk/`, `apps/`, `dapps/`, `kiosk/`

Review for correctness, API consistency, and security (XSS, injection). Documentation and minor style issues are non-blocking.

**Tier 5 — Light (docs, examples, tooling):**

- `docs/`, `examples/`, `scripts/`, `dev-tools/`, `docker/`, `setups/`

Flag factual errors or anything that would mislead a developer. Do not apply protocol-level scrutiny.

**Rule:** when a PR touches multiple tiers, apply the strictest relevant standard to the entire review.

---

## Cross-cutting checks

These apply regardless of language or tier.

**License headers (blocking):** New IOTA source files must have:

```
// Copyright (c) <year> IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
```

Files originating from Mysten Labs must add a modification line:

```
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) <year> IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
```

Valid years: 2024 through the current year. This is enforced by `linting/license-check/` for TypeScript; apply the same standard to Rust and Move files manually.

**Changesets (blocking for publishable SDK packages):** PRs modifying publishable packages under `sdk/` must include a `.changeset/` file with the correct semver bump. The following packages are excluded from this requirement: iota-wallet, iota-explorer, wallet-dashboard, apps-backend, iota-evm-bridge, @iota/core, sponsored-transactions, kiosk-demo, kiosk-cli, @iota/examples.

**PR description (blocking if absent):** Must explain _why_ the change was made. A reviewer cannot assess correctness without understanding intent. If the reason is not obvious from the code and the description is missing, note this explicitly.

**Breaking changes (blocking):** Public API changes (Rust, TypeScript, or Move) must be additive or follow a deprecation path. In Rust, items being removed go through `#[deprecated]` first. In TypeScript SDK, removed exports require a major version bump via changeset.

**Test coverage:** Tiers 1–3 require tests covering failure modes and boundary conditions, not just the happy path. Tier 4 expects SDK functions tested against realistic inputs. Tier 5 tests are nice-to-have.

**Dependency hygiene (blocking if unjustified):** New dependencies (Rust crates or npm packages) require justification. Flag unmaintained or vulnerable dependencies as blocking regardless of justification.

---

## Rust review

Apply all rules from `RUST_CONVENTIONS.md`. The following are additional review-specific checks not covered there.

**Diff scan patterns — flag these in the diff:**

- `unwrap()` outside test code (must use `.expect()` with a meaningful message or propagate)
- `unsafe` without a justifying comment
- `panic!` / `unreachable!` / `.expect()` in Tier 1–2 code without a `# Panics` rustdoc section
- `use something::*` outside public re-exports
- `super::` imports outside test modules
- New entries in `Cargo.toml` `[dependencies]` — verify `workspace = true` and `default-features = false` where applicable
- `#[allow(..)]` attributes — require a comment explaining why the lint is suppressed
- `#[from]` in `thiserror` definitions — prefer `#[source]` with manual context

**Protocol config changes:** In `crates/iota-protocol-config/`, new fields must have defaults that preserve existing behavior. Snapshot tests must be updated.

**Execution versioning:** `iota-execution/src/lib.rs` is generated by `./scripts/execution-layer`. New execution versions are added as new match arms; existing versions must not be modified.

**Formatting is not a review concern:** `cargo +nightly fmt` with `rustfmt.toml` settings is enforced by CI.

---

## TypeScript / Frontend review

**Blocking:**

- `Buffer` usage in browser code outside `sdk/ledgerjs-hw-app-iota/` and `apps/wallet/` (causes bundle bloat, breaks web compatibility)
- Missing license headers on new files
- Circular imports in SDK packages (`import/no-cycle` is enforced)
- Missing `.js` file extensions in `sdk/` package imports (ESM requirement enforced by `import/extensions`)
- `dangerouslySetInnerHTML` usage in explorer (XSS risk)

**Non-blocking:**

- `any` type usage in SDK code — flag gratuitous cases
- Inconsistent type imports — should use `import type` or `import { type X }`
- Explorer components: prefer function declarations over arrow functions; use `LinkWithQuery` / `useSearchParamsMerged` / `useNavigateWithQuery` over bare react-router-dom equivalents
- `console.log` / `console.warn` / `console.error` in wallet production code
- Missing input validation in SDK public functions
- Bundle size regressions from new dependencies in `apps/`
- Test framework consistency: Vitest for `sdk/` and `apps/`, Jest for `apps-backend`

**Not a review concern:** Prettier formatting (printWidth 100, tabWidth 4, singleQuote, trailingComma all) and Tailwind class sorting are enforced by CI.

**SDK API design:** New exports should follow the existing subpath pattern (`./bcs`, `./client`, `./transaction`, etc.). Type-only exports should use `export type`.

---

## Move smart contract review

Move code appears in three areas with different severity levels:

- `crates/iota-framework/packages/` — Tier 1–2 (system contracts)
- `external-crates/move/crates/` — Tier 2 (Move language implementation)
- `examples/move/`, `dapps/` — Tier 5 (but flag anti-patterns that users might copy)

**Blocking (framework and production code):**

- **Access control:** `public entry` functions modifying state must validate caller authority via capabilities (`TreasuryCap`, `AdminCap`, etc.) or appropriate ownership checks
- **Object safety:** Structs with the `key` ability must have `id: UID` as their first field; verify `transfer` vs `public_transfer` is appropriate for the object's intended transferability
- **One-time witness (OTW) patterns:** Must have only the `drop` ability, uppercase module name as the type name, and no fields
- **Shared objects:** Flag new shared objects in framework code — they create contention and are permanent once created
- **Error codes:** Use descriptive constant names (e.g., `ENotEnough`, `EBadWitness`), not raw integer literals at abort points
- **Event emission:** State changes that clients or indexers depend on should emit events
- **Upgrade safety:** No removing struct fields, no changing public function signatures in published packages

**Non-blocking:**

- Missing edge-case tests (zero values, max values, empty collections)
- Missing doc comments on public functions and structs
- Using `vector` where `Table` / `VecMap` / `VecSet` would be more gas-efficient at scale
- `#[expected_failure]` tests should specify `abort_code = module::ERROR_CONST` rather than a raw integer

**Testing:** Test helpers should live in `#[test_only]` modules. Tests should cover both success and failure paths with specific abort codes.

---

## What to check in every review

**Correctness.** Does the code do what the PR claims? Trace through failure cases, not just the happy path. In protocol code, assume adversarial inputs and network conditions.

**Error propagation.** Follow errors from origin to where they surface. Check that context is added at each layer. An error that says "IO error" when it reaches a user is missing context from every layer it passed through.

**API stability.** If the change touches a public interface (Rust, TypeScript, or Move), verify deprecation and versioning rules are followed.

**Concurrency and state.** In node code, watch for race conditions, deadlocks, and lock ordering issues. In React code, watch for stale closures over state and missing dependency arrays in hooks.

---

## Review output

After completing your review, you **must** post a review comment on the pull request. Every review produces a comment — no exceptions, even if there are no issues found.

### Comment structure

Your review comment must contain two parts:

**1. Summary of changes:** Start with a concise explanation of what the PR does and why. Describe the purpose and scope of the changes in 2–4 sentences so that other reviewers and the author can quickly understand the intent and impact. Reference the tier(s) of code affected.

**2. Review findings:** Group findings by severity — **blocking issues first**, then non-blocking suggestions. If there are no blocking issues and no non-blocking suggestions, explicitly state that the changes look good and the review found no issues.

For each finding: state the file and line, describe the problem precisely, explain why it matters, and suggest what a fix looks like. Do not leave a comment that says something is wrong without explaining what better looks like.

### Additional guidelines

Do not flag formatting or style issues enforced by CI (`rustfmt`, Prettier, ESLint auto-fixable rules). These are not review concerns.

Acknowledge when you lack sufficient context to assess correctness deeply (e.g., consensus algorithm changes requiring knowledge of the protocol state machine). Say so clearly rather than producing a shallow review that appears thorough.

For mechanical PRs (dependency bumps, generated code, automated refactors), focus on whether the automation ran correctly and the result is consistent, rather than reviewing each line individually.
