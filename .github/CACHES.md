# GitHub Actions Cache Strategy

This document describes our shared-key caching strategy for GitHub Actions workflows. Jobs with compatible build artifacts share cache keys to maximize cache hits and reduce build times.

## Cache Key Overview

| Cache Key         | Description                                         | Jobs Using This Cache                                                                                                                                           |
| ----------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **rust-build**    | Main Rust workspace builds with stable toolchain    | • rust-tests<br/>• test-extra<br/>• e2e localnet<br/>• execution-cut<br/>• move-ide-test<br/>• rosetta<br/>• split-cluster<br/>• license-check<br/>• crate-docs |
| **rust-clippy**   | Clippy-specific metadata and linting artifacts      | • clippy                                                                                                                                                        |
| **rust-examples** | External examples with separate Cargo.toml files    | • lint-examples                                                                                                                                                 |
| **rust-udeps**    | Nightly toolchain builds for dependency analysis    | • check-unused-deps                                                                                                                                             |
| **rust-simtest**  | Simulation testing with patched dependencies        | • nightly simtest<br/>• nightly simtest-with-starfish<br/>• rust-simtests                                                                                       |
| **rust-llvm-cov** | Coverage builds with special CARGO_PROFILE settings | • llvm-cov                                                                                                                                                      |
| **default**       | OS-specific default builds (matrix differentiated)  | • release<br/>• release-move-ide                                                                                                                                |
