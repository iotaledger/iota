# non_exhaustive_lint

Nightly CI check that reports `iota-sdk-types` `#[non_exhaustive]` enum variants
which a `match` leaves to a wildcard (`_`) arm. When the SDK adds a variant, such
a wildcard swallows it silently instead of forcing the code to handle it; this
surfaces those spots.

## What it is (and is not)

- A **report**, run at warn level. A finding never fails the build. The job
  fails only its own self-test, i.e. when the lint has stopped being applied at
  all (misconfiguration), so a silent break cannot make CI pass everything.
- Not a hard gate. A gate would need the lint scoped to SDK enums only; the
  built-in `non_exhaustive_omitted_patterns` fires on every `#[non_exhaustive]`
  enum, so roughly half of a whole-workspace run is unrelated (grpc types, our
  own internal enums, third-party crates). Scoping it precisely would need a
  custom rustc driver reusing rustc's exhaustiveness engine, which a dylint lint
  cannot reach (THIR is dropped before late lints run). That is out of scope here.

## How it works

- `check.sh` derives the in-scope crate set as every workspace member that
  reaches `iota-sdk-types` in its dependency tree (`cargo tree --invert`), i.e.
  every crate that could name an SDK type, directly or through a re-export from
  another crate. Crates with no path to `iota-sdk-types` (e.g. `iota-multiaddr`,
  the proc-macro crates) are excluded, so their unrelated `#[non_exhaustive]`
  enums stay out. No skip list.
- The lint is unstable and nightly-only. `check.sh` runs one
  `cargo +nightly check --workspace --all-targets --all-features` with
  `rustc_wrapper.sh` set as `RUSTC_WORKSPACE_WRAPPER` (`--all-features` so a match
  behind a `#[cfg(feature = "...")]` that is off by default is still compiled and
  linted). The wrapper appends the lint's crate attributes
  (`feature(non_exhaustive_omitted_patterns_lint)` and
  `warn(non_exhaustive_omitted_patterns)`) only when `CARGO_PKG_NAME` is in the
  in-scope list. It runs for workspace members only, so dependencies, including
  third-party crates with their own `#[non_exhaustive]` enums, are never
  injected. No crate source carries the attribute.
- The wrapper also passes `-Ztrim-diagnostic-paths=no`, so every type in a
  diagnostic carries its defining crate (`iota_sdk_types::Owner`, never a
  trimmed bare `Owner`). That is what makes the report's SDK classification
  exact rather than a list of bare names.

## Reading the results

`report.sh` turns the raw compiler output into a markdown report: one row per
source site (a crate's lib and test targets otherwise report the same site
twice), SDK matches first and split into production code and tests/examples,
and every other `#[non_exhaustive]` match (internal and third-party enums)
collapsed into per-type counts. A match is an SDK match iff its type mentions
`iota_sdk_types` or `iota_sdk_grpc`.

`check.sh` prints the report, appends it to the job summary page
(`GITHUB_STEP_SUMMARY`), and writes `report.md` next to the full
`cargo-check.log` into `NELINT_OUT_DIR`; CI uploads that directory as the
`non-exhaustive-lint-report` artifact.

## Known coverage gaps

Because this is a report rather than a gate, coverage is deliberately a
best-effort approximation. It does not lint:

- **Code behind an inverse or target cfg.** The run uses `--all-features`, so
  positive `#[cfg(feature = "...")]` gates are covered. Still not linted: `match`es
  behind `#[cfg(not(feature = "..."))]` (compiled only when a feature is off), and
  `#[cfg(target_...)]` code that does not apply to the CI host (e.g. wasm-only
  paths in the SDK crates, which would need a wasm cross-compile).
- **`external-crates/move`.** It is a separate Cargo workspace, so the root
  `cargo check --workspace` never builds it.
- **Doctests.** `--all-targets` does not build them.

Caching: the wrapper's inject-or-not decision is not part of cargo's fingerprint,
so a warm target dir can replay a crate's artifacts from before it entered scope
and silently skip linting it. CI avoids this by building in a fresh per-run
`CARGO_TARGET_DIR` (a full cold build, ~8 min). A local rerun on a warm target dir
may under-report after the in-scope set changes; delete the target dir (or the
affected crate's artifacts) to force a re-lint.

## Run locally

```sh
scripts/non_exhaustive_lint/check.sh
```

`NIGHTLY` overrides the toolchain (default matches CI). The report prints at the
end; set `NELINT_OUT_DIR=<dir>` to also get `report.md` and the full
`cargo-check.log` there. The self-test and a genuine build error are the only
things that make the script exit non-zero.
