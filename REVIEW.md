# REVIEW.md

This file instructs Claude Code on how to conduct code reviews in the IOTA monorepo. Read it fully before reviewing any pull request.

---

## Review depth by path

Not all code carries the same risk. Apply scrutiny accordingly.

**Protocol and consensus code** (`crates/` paths related to networking, consensus, ledger, or node operation) requires the deepest review. Bugs here can cause network faults, data loss, or security vulnerabilities. Treat every change as potentially load-bearing until proven otherwise. Missing tests, unjustified panics, or unclear error handling in these paths are blocking issues.

**Infrastructure and shared libraries** (common utilities, shared types, RPC interfaces, storage abstractions) deserves thorough review but with slightly more tolerance for pragmatic tradeoffs. API stability and correct error propagation matter a lot here because other crates depend on them.

**TypeScript SDKs and tooling** should be reviewed for correctness and API consistency, but documentation gaps, relaxed test coverage on utilities, and minor style issues are non-blocking. The primary concern is whether the SDK accurately and safely exposes the underlying protocol behavior.

**Documentation, examples, and configuration** can be reviewed lightly. Flag factual errors or anything that would mislead a developer, but do not apply protocol-level scrutiny.

When a PR touches multiple layers, apply the strictest relevant standard to the entire review.

---

## Blocking issues

These must be resolved before merge. Flag them explicitly.

Any use of `unsafe` without a comment justifying why it is sound and why a safe alternative was not viable. The justification must be in the code, not just the PR description.

Any `.unwrap()` outside of test code. Protocol and library code must use `.expect()` with a meaningful message, or propagate the error properly. An `.expect()` message that just restates the type ("expected Some value") is not sufficient. The message should explain what invariant was violated.

Any `panic!()`, `unreachable!()`, or `.expect()` in protocol code that lacks a `# Panics` section in the function's rustdoc. If the function can panic, it must say so and under what conditions.

Error types that use `#[from]` in `thiserror` definitions where the wrapped error provides insufficient context. Prefer `#[source]` with a manual mapping that adds diagnostic context at the call site.

New dependencies added without justification in the PR description. Flag any dependency that is not widely used in the Rust ecosystem and ask for evidence it has been vetted. Flag unmaintained crates as blocking regardless of justification.

Public API changes that are breaking without a corresponding deprecation path. New public items should be additive. Items being removed must go through `#[deprecated]` first with a reason and target version.

Missing or incorrect error handling in protocol paths where a failure should propagate rather than be swallowed. Silent failures in networking or consensus code are bugs.

Use of `use something::*` wildcard imports outside of public re-exports.

Use of `super::` for import paths outside of test modules.

Structs or enums in public APIs that are expected to grow but are not marked `#[non_exhaustive]`.

New `anyhow` usage in library code. `anyhow` is for applications, not libraries. Library error types must be matchable.

Tests that expose non-public APIs by making them `pub` rather than using `#[cfg(test)]`-gated APIs or defining tests locally.

---

## Non-blocking issues

Raise these as suggestions. They should be addressed but will not block merge.

Variable names that are too abbreviated or too generic in non-trivial contexts. Single-letter names outside of loop indexes and single-line closures are worth flagging.

`mod.rs` files that contain significant logic rather than just re-exports. This is a recommendation, not a hard rule, but worth noting if a file is growing.

Missing contextual detail in error messages. Short messages are fine; uninformative ones are not. If an error message would not help a developer debug the problem quickly, suggest improving it.

Public APIs that lack accompanying examples in the rustdoc, particularly if the usage is non-obvious.

Generics or lifetimes in public APIs using single-letter names where a descriptive name would make the contract clearer (e.g. `'data` instead of `'a`, `Doc` instead of `D`).

High-level library functionality that is not documented in `lib.rs`.

TypeScript SDK functions that do not validate inputs before passing them to protocol-level calls.

---

## What to check in every review

**Correctness.** Does the code do what the PR claims? Trace through the logic for the failure cases, not just the happy path. In protocol code, assume adversarial inputs and network conditions.

**Tests.** Protocol and library changes must have tests covering meaningful behavior, including failure modes and boundary conditions. A test that only exercises the happy path on a function that can fail is not sufficient coverage. In TypeScript, check that SDK behavior is tested against realistic inputs. Use `.unwrap()` in test code rather than returning `Result`, so stack traces are visible on failure.

**Error propagation.** Follow errors from where they originate to where they surface. Check that context is added at each layer, not just at the origin. An error that says "IO error" by the time it reaches a user is missing context from every layer it passed through.

**API stability.** If the change touches a public interface, verify the deprecation and versioning rules are followed. Check that `Cargo.toml` dependency versions are as fine-grained as is non-breaking per SemVer, that `default-features = false` is set for workspace dependencies, and that workspace-level dependencies use `workspace = true` in crate manifests.

**Dependency hygiene.** Any new dependency warrants scrutiny. Check whether it is maintained, whether a more widely-used alternative exists, and whether a lighter approach was considered.

**Panic safety.** Search the diff for `unwrap`, `expect`, `panic!`, `unreachable!`, and `unsafe`. Each one needs a justification comment. In protocol code, treat unjustified panics as bugs.

---

## How to structure your output

Group findings by severity: blocking issues first, then non-blocking suggestions. For each finding, state the file and line, describe the problem precisely, explain why it matters, and suggest what a fix looks like. Do not leave a comment that says something is wrong without explaining what better looks like.

Do not flag style issues that are not covered by the conventions in this file. Do not comment on formatting: that is enforced by `cargo +nightly fmt` and is not a review concern.

If the PR description does not explain why a change was made and the reason is not obvious from the code, note this explicitly. A reviewer cannot assess correctness without understanding intent.

If a change is in an area where you lack sufficient context to assess correctness (for example, a consensus algorithm change that requires deep knowledge of the protocol state machine), say so clearly rather than producing a shallow review that appears thorough.
