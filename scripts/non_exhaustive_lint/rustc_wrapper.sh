#!/usr/bin/env bash
# RUSTC_WORKSPACE_WRAPPER for the non_exhaustive_omitted_patterns check.
#
# Cargo invokes this as: rustc_wrapper.sh <real-rustc> <rustc-args...>, and only
# for workspace members (never registry/git dependencies). When the crate being
# compiled is in scope, we append the lint's two crate attributes; every other
# crate compiles untouched.
#
# Scope is keyed on CARGO_PKG_NAME, which Cargo sets in the environment of each
# rustc invocation. NELINT_PKGS_FILE lists the in-scope package names, one per
# line. Keying on the crate's own identity means a dependency of an in-scope
# crate is not affected, so third-party #[non_exhaustive] enums in the
# dependency graph never trip the lint, and no crate source is edited.
set -u

rustc="$1"; shift

pkg="${CARGO_PKG_NAME:-}"
# The two diagnostics flags make every type from another crate carry that crate
# in a warning (iota_sdk_types::Owner, never a bare Owner). rustc shortens type names in two
# independent ways: -Ztrim-diagnostic-paths=no disables the shortest-unambiguous-
# name trimming, and -Zwrite-long-types-to-disk=no disables the abbreviation of
# long types (which otherwise drops crate paths above ~2/3 of the diagnostic
# width). report.sh classifies and findings.sh keys on those full paths.
if [[ -n "$pkg" && -n "${NELINT_PKGS_FILE:-}" && -f "$NELINT_PKGS_FILE" ]] \
   && grep -qxF -- "$pkg" "$NELINT_PKGS_FILE"; then
  exec "$rustc" "$@" \
    "-Zcrate-attr=feature(non_exhaustive_omitted_patterns_lint)" \
    "-Zcrate-attr=${NELINT_LEVEL:-warn}(non_exhaustive_omitted_patterns)" \
    "-Ztrim-diagnostic-paths=no" \
    "-Zwrite-long-types-to-disk=no"
fi

exec "$rustc" "$@"
