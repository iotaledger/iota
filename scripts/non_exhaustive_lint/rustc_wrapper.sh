#!/usr/bin/env bash
# RUSTC_WORKSPACE_WRAPPER for the non_exhaustive_omitted_patterns check. Cargo
# runs it as `rustc_wrapper.sh <rustc> <args...>` for workspace members only,
# with CARGO_PKG_NAME in the environment; NELINT_PKGS_FILE lists the in-scope
# package names.
set -u

rustc="$1"; shift

pkg="${CARGO_PKG_NAME:-}"
# rustc shortens type names in two independent ways: -Ztrim-diagnostic-paths=no
# turns off the shortest-unambiguous-name trimming, -Zwrite-long-types-to-disk=no
# the abbreviation of long types (above about 2/3 of the diagnostic width), which
# drops crate paths. findings.sh relies on the full paths.
if [[ -n "$pkg" && -n "${NELINT_PKGS_FILE:-}" && -f "$NELINT_PKGS_FILE" ]] \
   && grep -qxF -- "$pkg" "$NELINT_PKGS_FILE"; then
  exec "$rustc" "$@" \
    "-Zcrate-attr=feature(non_exhaustive_omitted_patterns_lint)" \
    "-Zcrate-attr=warn(non_exhaustive_omitted_patterns)" \
    "-Ztrim-diagnostic-paths=no" \
    "-Zwrite-long-types-to-disk=no"
fi

exec "$rustc" "$@"
