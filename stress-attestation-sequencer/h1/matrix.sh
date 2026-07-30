#!/usr/bin/env bash
#
# matrix.sh — run the H1 attestation-overhead matrix (V1 vs V2), ITERS iterations
# each, as labeled experiments under results/<LABEL>/.
#
# Two workload families. Neither takes a MUTABLE shared object, so per-object
# congestion control never accumulates cost and no deferral or cancellation
# confounds the V1-vs-V2 delta (the congestion tracker filters on `obj.mutable`).
# They sweep OPPOSITE sides of attestation:
#
# A. slow (45 configs) — what attestation COSTS. Owned-object inputs only
#    (SLOW_SHARED=false), so these txs have no shared inputs at all.
#    Each tx calls slow::slow(n, size) with n == size. A `slow` tx carries no
#    MoveAuthenticator, so post-consensus Check #6 skips the Move VM on BOTH arms
#    (the `!move_authenticators.is_empty()` guard in authority.rs) and the cost
#    lands in execution, which both arms pay on every validator. Raising SLOW_N
#    therefore grows a cost V1 and V2 share, plus the pre-consensus dry-run that
#    only V2 adds.
#
#      5 compute {0, 50, 100, 200, 500}  (0 = no-op floor, ~gas_rounding_step)
#    × 3 paths   {f1 fullnode (DIRECT=false), v1 pinned (1 target validator),
#                 v4 spread (direct to all 4 validators)}
#    × 3 qps     {200, 1000, 2000}                                   = 45 configs.
#
# B. moveauth (45 configs) — what attestation SAVES.
#    Every tx is signed with a `MoveAuthenticator` (account abstraction) over an
#    owned-object coin split/transfer body, using the `ed25519heavy` authenticate
#    function. V1 executes that function in the Move VM during post-consensus
#    Check #6 — once per validator, serially inside the consensus handler — and
#    V2 skips the call entirely, so AUTH_CYCLES sweeps a cost only V1 pays.
#
#      5 cycles {1, 5, 10, 20, 50}  (ed25519 verifications per tx; geometric,
#                                    matching family A's spacing)
#    × 3 paths  {f1, v1, v4 as above}
#    × 3 qps    {200, 1000, 2000}                                    = 45 configs.
#
#    `ed25519heavy` takes no object inputs, so its whole cost is Move VM work in
#    the authenticate call — nothing is added to the post-consensus baseline that
#    both arms pay. That is why it is used here rather than `superheavy`, whose
#    122 BenchObject inputs are loaded by BOTH arms (V2 loads them too, via
#    check_coin_deny_list_for_attested_tx) and would dilute the delta while adding
#    ~9 KB per transaction.
#
#    All cells are NON-FAILING: AUTH_SHOULD_FAIL=false (and `ed25519heavy` does
#    not assert the verification result anyway), and the cycle counts stay well
#    under the budget ceiling. One verification costs ~2800-3300 gas, so at the
#    privnet's max_auth_gas=250000 roughly 89 fit; the top rung of 50 is ~56% of
#    that. That margin is deliberate — the per-cycle figure is extrapolated from a
#    measured `superheavy` sweep, not measured for `ed25519heavy`, so a rung close
#    to the ceiling risks aborting outright if the real cost is higher. Raising
#    AUTH_CYCLES past the ceiling aborts every tx with OUT_OF_GAS, which measures
#    admission pressure instead of the check cost — if max_auth_gas is overridden,
#    rescale these counts with it.
#
#    AUTH_CYCLES=1 is the light baseline rung. The other kinds are unusable here:
#    helloworld / ed25519 / maxargs125 have constant Move loops that cannot be
#    dialled, and both superheavy kinds abort by construction (100M verifications
#    / u256::MAX). Only kinds whose Move function takes the count as a call
#    argument respond to AUTH_CYCLES — see AuthenticatorKind::takes_cycle_count in
#    the network-benchmark repo.
#
#    NOTE: `-owned-` in these labels describes the transaction BODY
#    (AUTH_OBJ_TYPE=owned-object, a coin split/transfer). Unlike family A these
#    txs are NOT free of shared inputs: the authenticator names the shared
#    `AbstractAccount` object, and SenderSignedData::shared_input_objects folds
#    authenticator shared objects into the tx's shared-object set, so
#    contains_shared_object() is true and they take the shared-object path through
#    consensus handling. It is a read-only input (mutable: false), which is why
#    congestion control still stays out of the comparison.
#
#    Labels are auth<cycles> (auth1, auth5, auth10, auth20, auth50). FILTER is a
#    plain substring match, so "auth1" also selects auth10 and "auth5" also
#    selects auth50 — add the trailing hyphen ("auth1-", "auth5-") to pick exactly
#    one level.
#
# 90 configs total. Use the substring FILTER to run one family at a time
# (`./matrix.sh auth`, `./matrix.sh slow`) — the full grid at ITERS=5 is days of
# wall time.
#
# Labels carry the network size as an -n<N> suffix and pass N to run.sh; the
# current grid runs on a 4-validator network (-n4 / N=4). The same grid can be
# run on another size (e.g. -n24 / N=24) under distinct labels without colliding
# with these results.
#
# Round-robin: each round runs 1 iteration (V1+V2) of every config; ITERS rounds
# total, so each config ends with ITERS iters — interleaved, not config-major. So
# an interrupted run leaves every config with ~equal iters. That's 90 * ITERS full
# experiments — DAYS of wall time at ITERS=5 for the whole grid, so filter to one
# family unless you mean it. Per-config console output goes to
# logs/<LABEL>.log (truncated on round 1, appended thereafter); redirecting it also
# makes run.sh non-interactive (no monitoring prompt) and strips ANSI colors, so
# the matrix runs unattended.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 90 configs (both families)
#   ITERS=5 ./matrix.sh auth        # only the 45 moveauth configs
#   ITERS=5 ./matrix.sh slow        # only the 45 slow configs
#   ITERS=3 ./matrix.sh auth50-     # one cycle count, all paths/qps (9 configs)
#   ITERS=3 ./matrix.sh auth1-      # trailing hyphen: auth1 only, not auth10
#   ITERS=3 ./matrix.sh auth5-      # trailing hyphen: auth5 only, not auth50
#   ITERS=3 ./matrix.sh slow100     # only labels containing "slow100" (substring filter)
#
# A config that fails (or is interrupted) does NOT abort the matrix — it's logged
# and the next config runs. Re-running is safe: run.sh's config gate appends more
# iterations to an existing label (same config) rather than overwriting.
#
# Per-config runs skip aggregate/plots (ANALYZE=false): those tools re-read every
# accumulated iteration of a label, so invoking them each round costs
# quadratically over a campaign. One sweep after the last round aggregates +
# plots every label; if the matrix is interrupted before it, run the sweep
# manually (see the bottom of this script).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ITERS="${ITERS:-5}"
FILTER="${1:-}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# Node-log compression is CPU-bound and single-threaded under gzip; use pigz
# (parallel gzip, same .gz format) when installed.
GZIP_BIN="$(command -v pigz || echo gzip)"

# When launched detached (nohup / output not a terminal), send our own console
# output to logs/_matrix.log instead of relying on an outer `> logs/_matrix.log`
# redirect — that redirect is opened by the shell before this script's mkdir runs,
# so it would fail if logs/ didn't exist yet. Now `nohup ./matrix.sh &` just works.
# (Per-config detail still goes to logs/<LABEL>.log.)
if [[ ! -t 1 ]]; then
  exec >"$LOGDIR/_matrix.log" 2>&1
fi

# "LABEL | env assignments passed to run.sh"
configs=(
  "slow0-owned-f1-qps200-n4    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=false"
  "slow0-owned-v1-qps200-n4    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v4-qps200-n4    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow0-owned-f1-qps1000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=false"
  "slow0-owned-v1-qps1000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v4-qps1000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow0-owned-f1-qps2000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=false"
  "slow0-owned-v1-qps2000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v4-qps2000-n4   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "slow50-owned-f1-qps200-n4   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=false"
  "slow50-owned-v1-qps200-n4   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v4-qps200-n4   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow50-owned-f1-qps1000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=false"
  "slow50-owned-v1-qps1000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v4-qps1000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow50-owned-f1-qps2000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=false"
  "slow50-owned-v1-qps2000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v4-qps2000-n4  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "slow100-owned-f1-qps200-n4  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=false"
  "slow100-owned-v1-qps200-n4  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v4-qps200-n4  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow100-owned-f1-qps1000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=false"
  "slow100-owned-v1-qps1000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v4-qps1000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow100-owned-f1-qps2000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=false"
  "slow100-owned-v1-qps2000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v4-qps2000-n4 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "slow200-owned-f1-qps200-n4  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=false"
  "slow200-owned-v1-qps200-n4  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v4-qps200-n4  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow200-owned-f1-qps1000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=false"
  "slow200-owned-v1-qps1000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v4-qps1000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow200-owned-f1-qps2000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=false"
  "slow200-owned-v1-qps2000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v4-qps2000-n4 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "slow500-owned-f1-qps200-n4  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=false"
  "slow500-owned-v1-qps200-n4  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v4-qps200-n4  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow500-owned-f1-qps1000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=false"
  "slow500-owned-v1-qps1000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v4-qps1000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "slow500-owned-f1-qps2000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=false"
  "slow500-owned-v1-qps2000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v4-qps2000-n4 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"

  #         non-failing. Sweeps the Move VM cost that only the V1 arm pays
  #         post-consensus. Ordered cheapest -> costliest authenticate function.
  #
  #
  #
  #
  # Object-loading control (empty Move body, 122 authenticator input objects), not
  # a point on the compute ladder — see the header.

  # ---- B. moveauth: ed25519heavy authenticator, owned-object body,
  #         non-failing. AUTH_CYCLES ed25519 verifications per tx is the cost
  #         dial for the Move VM work only the V1 arm pays post-consensus.
  #
  "auth1-owned-f1-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=false"
  "auth1-owned-v1-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth1-owned-v4-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth1-owned-f1-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=false"
  "auth1-owned-v1-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth1-owned-v4-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth1-owned-f1-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=false"
  "auth1-owned-v1-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth1-owned-v4-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=1 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "auth5-owned-f1-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=false"
  "auth5-owned-v1-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth5-owned-v4-qps200-n4    | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth5-owned-f1-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=false"
  "auth5-owned-v1-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth5-owned-v4-qps1000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth5-owned-f1-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=false"
  "auth5-owned-v1-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth5-owned-v4-qps2000-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=5 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "auth10-owned-f1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=false"
  "auth10-owned-v1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth10-owned-v4-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth10-owned-f1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=false"
  "auth10-owned-v1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth10-owned-v4-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth10-owned-f1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=false"
  "auth10-owned-v1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth10-owned-v4-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=10 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "auth20-owned-f1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=false"
  "auth20-owned-v1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth20-owned-v4-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth20-owned-f1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=false"
  "auth20-owned-v1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth20-owned-v4-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth20-owned-f1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=false"
  "auth20-owned-v1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth20-owned-v4-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=20 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  #
  "auth50-owned-f1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=false"
  "auth50-owned-v1-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth50-owned-v4-qps200-n4   | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=200 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth50-owned-f1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=false"
  "auth50-owned-v1-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth50-owned-v4-qps1000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=1000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
  "auth50-owned-f1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=false"
  "auth50-owned-v1-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "auth50-owned-v4-qps2000-n4  | WORKLOAD=moveauth AUTHENTICATOR=ed25519heavy AUTH_CYCLES=50 AUTH_OBJ_TYPE=owned-object AUTH_SHOULD_FAIL=false TARGET_QPS=2000 N=4 DIRECT=true NUM_TARGET_VALIDATORS=4"
)

# Cache sudo up front (run.sh uses sudo per iteration) and keep it alive for the
# whole matrix — otherwise creds expire mid-run and sudo blocks on /dev/tty.
sudo -v || {
  echo "matrix.sh: need sudo (run.sh uses it for cleanup/bootstrap)"
  exit 1
}
(while true; do
  sudo -n true
  sleep 60
  kill -0 "$$" 2>/dev/null || exit
done) &
trap 'kill %1 2>/dev/null' EXIT

# Count filter-matching configs up front so the progress display knows the total.
nconf=0
for row in "${configs[@]}"; do
  l="${row%%|*}"
  l="${l// /}"
  [[ -n "$FILTER" && "$l" != *"$FILTER"* ]] && continue
  nconf=$((nconf + 1))
done
total=$((nconf * ITERS))
n=0
ok=0
fail=0
start=$(date +%s)
# Round-robin: each round runs ONE iteration of every config; ITERS rounds total,
# so each config ends with ITERS iters but they are interleaved. An interrupted
# matrix then leaves every config with ~equal iterations, instead of the first
# configs fully done and the last ones with none. (run.sh's config gate appends
# each round's iter to results/<LABEL>/.)
for ((round = 1; round <= ITERS; round++)); do
  echo "########## round $round of $ITERS ##########"
  for row in "${configs[@]}"; do
    label="${row%%|*}"
    label="${label// /}" # strip alignment padding around |
    envs="${row#*|}"
    [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && continue
    n=$((n + 1))
    log="$LOGDIR/$label.log"
    # Fresh per-config log on this invocation's first round, then append rounds 2..N.
    [[ $round -eq 1 ]] && : >"$log"
    echo "[$(date +%H:%M:%S)] ($n/$total) round $round  $label  -> logs/$label.log"
    echo "===== round $round =====" >>"$log"
    # shellcheck disable=SC2086  # $envs is intentionally word-split into KEY=VAL args
    if env LABEL="$label" ITERS=1 ANALYZE=false $envs "$SCRIPT_DIR/run.sh" >>"$log" 2>&1; then
      echo "    ✓ done"
      ok=$((ok + 1))
    else
      rc=$?
      echo "    ✗ FAILED (exit $rc) — tail $log"
      fail=$((fail + 1))
    fi
    # Compress the node logs this iteration captured (gzip ≈10:1) so a long
    # campaign does not fill the disk. _state.log/_crash.log stay uncompressed —
    # the crash scan reads them; the analysis tooling never reads node logs.
    sudo find "$SCRIPT_DIR/results/$label" -path '*node-logs/*.log' \
      ! -name '_state.log' ! -name '_crash.log' -exec "$GZIP_BIN" -f {} + 2>/dev/null
  done
done

# Aggregate + plot every label ONCE, now that all rounds are in (per-round
# analysis was skipped via ANALYZE=false above). If the matrix was interrupted
# before reaching this sweep, run it manually per label:
#   python3 aggregate.py results/<LABEL> results/<LABEL>/summary.md
#   .venv/bin/python plot.py --label <LABEL>
echo
echo "########## aggregate + plots (all labels) ##########"
VENV_PY="$SCRIPT_DIR/.venv/bin/python"
[[ -x "$VENV_PY" ]] ||
  echo "venv not found ($VENV_PY) — plots will be skipped (pip install matplotlib numpy)"
for row in "${configs[@]}"; do
  label="${row%%|*}"
  label="${label// /}"
  [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && continue
  [[ -d "$SCRIPT_DIR/results/$label" ]] || continue
  echo "[$(date +%H:%M:%S)] $label"
  python3 "$SCRIPT_DIR/aggregate.py" "$SCRIPT_DIR/results/$label" "$SCRIPT_DIR/results/$label/summary.md" ||
    echo "    ✗ aggregate.py failed"
  if [[ -x "$VENV_PY" ]]; then
    "$VENV_PY" "$SCRIPT_DIR/plot.py" --label "$label" ||
      echo "    ✗ plot.py failed"
  fi
done

mins=$((($(date +%s) - start) / 60))
echo
echo "matrix complete: $ok ok, $fail failed (of $n) in ${mins}m"
echo "results -> results/<LABEL>/  (summary.md + plots/ per label)"
