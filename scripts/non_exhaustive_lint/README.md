# non_exhaustive_lint

CI check that catches `iota-sdk-types` `#[non_exhaustive]` enum variants which a
`match` leaves to a wildcard (`_`) arm. When the SDK adds a variant, such a
wildcard swallows it silently instead of forcing the code to handle it. The check
compares the current findings with a committed allowlist and fails on any finding
that is not allowed, so a variant newly left to a wildcard, or a new wildcard
match, has to be looked at by a person.

## How the gate works

- The job runs in the nightly workflow, and on demand via `workflow_dispatch`.
  It does not run on pull requests, so a change shows up in the next nightly.
- The compiler lint `non_exhaustive_omitted_patterns` runs at warn level. A
  finding by itself never fails the job as configured (it would under
  `-D warnings`, which the check does not set).
- `findings.sh` reduces the findings to one line per distinct
  `file | matched type | variants not covered | sites=N`, sorted, with no line
  numbers. An edit that only moves code changes nothing. The flip side:
  replacing one wildcard match by another in the same file, over the same type,
  with the same variants not covered, keeps `sites=N` and is not noticed.
- The lint reports two shapes and both are kept: an enum `match` whose wildcard
  arm covers variants ("some variants are not matched explicitly"), and a struct
  pattern whose `..` covers fields of a `#[non_exhaustive]` struct ("some fields
  are not explicitly listed"). The third column is what rustc lists as not
  covered, variants or fields.
- `check.sh` compares the findings with the committed
  `scripts/non_exhaustive_lint/allowlist.txt` (comment lines and a trailing
  `# reason` on a line ignored, both sides sorted) and exits non-zero on any
  difference, printing the diff (also onto the job summary page). `+` is a
  finding that is not allowed, `-` is an allowlist entry that is no longer found.
- To allow a finding, add its exact `+` line to `allowlist.txt`, in any position,
  optionally followed by `# <why it is acceptable, or the issue that tracks it>`,
  and commit. To allow everything currently reported (this drops the reasons):

  ```sh
  scripts/non_exhaustive_lint/check.sh --allow-all
  git add scripts/non_exhaustive_lint/allowlist.txt
  ```

  The allowlist was seeded with the findings on third-party and internal enums
  only; the SDK findings were left out on purpose, so the job is red until each
  of them is fixed or allowed by hand.

## When the job is red, and why

Because the code changed (what the check is for):

- An in-scope `#[non_exhaustive]` enum gained a variant: every site that now
  leaves it to the wildcard changes its "variants not covered" text (a listed
  name, or rustc's "and N more" count). The old line shows as `-`, the new one
  as `+`. Handle the variant at each listed match.
- A new wildcard match appeared: a new `+` line, or a `+`/`-` pair with a higher
  `sites=` count.
- A site was fixed: a `-` line, or a pair with a lower count. Remove or update
  the allowlist entry; it keeps the file honest.
- A crate entered or left scope (gained or lost a path to `iota-sdk-types`).

For a cheap allowlist update after a glance:

- A file with matches was renamed or moved (the key carries the path).
- A third-party dependency bump changed one of its `#[non_exhaustive]` enums.
- The pinned nightly was bumped and rustc prints these diagnostics differently.

Because the tool or the environment broke, each with its own `ERROR:` line at
the end of the log:

- Self-test: the run produced no `non_exhaustive_omitted_patterns` finding at
  all, so the lint is no longer being injected. This runs before the allowlist
  comparison, so a dead wrapper is never reported as "every entry disappeared".
- The parser could not read a finding (rustc changed the diagnostic wording), or
  produced no findings.
- The scope could not be derived, or came out empty (`cargo metadata`,
  `cargo tree`, `jq` or `comm` failed).
- `cargo check` itself failed on the pinned nightly, or the toolchain or `jq` is
  missing on the runner.

Not red: a run whose findings all have an allowlist entry, including code that
only moved to other lines.

## How the findings are produced

- `check.sh` derives the in-scope crate set as every workspace member that
  reaches `iota-sdk-types` in its dependency tree (`cargo tree --workspace
  --invert --edges all --all-features`), i.e. every crate that could name an SDK
  type, directly or through a re-export from another crate. Crates with no path
  to `iota-sdk-types` (e.g. `iota-multiaddr`, the proc-macro crates) are
  excluded, so their unrelated `#[non_exhaustive]` enums stay out. No skip list:
  a crate that only matches third-party enums today may add an SDK match
  tomorrow.
- The lint is unstable and nightly-only. `check.sh` runs one
  `cargo +nightly check --workspace --all-targets --all-features` with
  `rustc_wrapper.sh` set as `RUSTC_WORKSPACE_WRAPPER` (`--all-features` so a
  match behind a `#[cfg(feature = "...")]` that is off by default is still
  compiled and linted). The wrapper appends the lint's crate attributes
  (`feature(non_exhaustive_omitted_patterns_lint)` and
  `warn(non_exhaustive_omitted_patterns)`) only when `CARGO_PKG_NAME` is in the
  in-scope list. It runs for workspace members only, so dependencies, including
  third-party crates with their own `#[non_exhaustive]` enums, are never
  injected. No crate source carries the attribute.
- The wrapper also passes `-Ztrim-diagnostic-paths=no` and
  `-Zwrite-long-types-to-disk=no`. rustc shortens type names in two independent
  ways (the shortest unambiguous name, and an abbreviation of long types that
  drops crate paths); with both off, every type from another crate carries that
  crate in the diagnostic (the compiled crate's own types print without a crate
  prefix). The allowlist keys and the report's SDK classification depend on that.

## Reading the results

`report.sh` turns the raw compiler output into a markdown report for people: one
row per source site (a crate's lib and test targets otherwise report the same
site twice), SDK matches first and split into production code and
tests/examples, and every other `#[non_exhaustive]` match (internal and
third-party enums) collapsed into per-type counts. A match is an SDK match iff
its type mentions `iota_sdk_types` or `iota_sdk_grpc`.

`check.sh` prints the report and appends it to the job summary page
(`GITHUB_STEP_SUMMARY`), and writes `report.md`, `findings.txt`,
`in-scope-crates.txt` and the full `cargo-check.log` into `NELINT_OUT_DIR`; CI
uploads that directory as the `non-exhaustive-lint-report` artifact.

## Known coverage gaps

Not linted by rustc. The lint checks wildcards in `match` patterns and nothing
else, so every other way of testing an SDK enum for one variant sends future
variants to its else side unseen:

- **`if let`, `let ... else`, `while let`.** No finding, including
  `if let Sdk::A = x { .. } else { unreachable!() }`. There is no exhaustive form
  of these; the only fix is an explicit `match`, which the lint then sees.
- **`matches!`.** On the pinned nightly, `matches!(x, Sdk::Variant)` over a
  `#[non_exhaustive]` enum produces no finding, for `x`, `&x`, `*x` and
  `Option<_>` alike, while the equivalent explicit `match x { Sdk::Variant =>
  .., _ => .. }` does, and so does a byte-identical local `macro_rules!`. Observed,
  not explained; the mechanism in rustc was not identified.

A grep over the SDK enum names (`grep -rnE '^\s*(if let|matches!\(|let .* else)' ...`)
counts such lines; when this was written: `matches!` 121 (28 in production
paths), `if let` 54 (23), `let else` 24 (13). Enforcing "explicit `match` on SDK
enums" would need a different lint (a HIR-level check for a refutable single
pattern over a foreign `#[non_exhaustive]` enum, feasible with dylint); not
built.

- **Matches built inside a cross-crate macro.** A `macro_rules!` from another
  crate whose scrutinee is constructed in the macro body is not linted; with a
  call-site scrutinee (`$e`) it is.

Coverage follows what the compiler compiles. Not linted:

- **Code behind an inverse cfg.** The run uses `--all-features`, so positive
  `#[cfg(feature = "...")]` gates are covered, but code behind
  `#[cfg(not(feature = "..."))]` is compiled only when that feature is off and is
  never seen. `grep -rn 'cfg(not(feature' crates iota-execution` lists those
  sites; when this was written there were two, both `tracing`, neither
  containing a `match`. If one of them grows a `match` on an SDK enum, a second
  pass with `--no-default-features` (union of both logs) would close the gap.
- **Simulator-only code.** `#[cfg(msim)]` code (about 120 blocks when this was
  written) is compiled only by `cargo simtest`, which this check does not run.
- **Code behind a target cfg** that does not apply to the CI host, e.g.
  wasm-only paths in the SDK crates, which would need a wasm cross-compile.
- **`external-crates/move`.** It is a separate Cargo workspace, so the root
  `cargo check --workspace` never builds it. It has no dependency on
  `iota-sdk-types` today.
- **Doctests.** `--all-targets` does not build them.

Limits of the key itself:

- A `match` inside a local macro whose scrutinee is built in the macro body is
  reported at the macro definition, so all its expansions count as one site.
- rustc lists the first three variants not covered and a count ("and N more"),
  in declaration order. An SDK change that adds one variant and removes another,
  both beyond the third, leaves the text unchanged.

Caching: the wrapper's inject-or-not decision is not part of cargo's fingerprint,
so a warm target dir can replay a crate's artifacts from before it entered scope
and silently skip linting it. CI avoids this by building in a fresh per-run
`CARGO_TARGET_DIR` (a full cold build, about 2 minutes on the self-hosted
runner). A local rerun on a warm target dir may under-report after the in-scope
set or the wrapper flags changed; use a fresh target dir for those.

## Run locally

```sh
scripts/non_exhaustive_lint/check.sh              # compare with the committed allowlist
scripts/non_exhaustive_lint/check.sh --allow-all  # rewrite the allowlist with every current finding
```

`NIGHTLY` overrides the toolchain (default matches CI). Set
`NELINT_OUT_DIR=<dir>` to also get `report.md`, `findings.txt`,
`in-scope-crates.txt` and the full `cargo-check.log` there. Exit codes: 0 when
the findings match the allowlist (or `--allow-all` wrote it), 2 usage, cargo's
own code when `cargo check` fails, 1 for everything else (findings not allowed,
missing allowlist, self-test, parser, scope).
