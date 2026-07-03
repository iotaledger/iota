# Stress-test runs and results

Running log of the stress tests from `stress-plan.md`: the exact commands,
the results of each run, and a brief analysis.

All commands are run from the `iota` monorepo root unless noted.

---

## H1 — attestation overhead (W4: slow owned-object; V1 vs V2)

**Goal.** Measure what attestation costs. Attestation (the pre-consensus
dry-run) happens in the `submit_tx` path independent of the congestion mode, so
H1 deliberately keeps sequencing out of the picture: **owned-object**
transactions only (no shared-object scheduling at all). Each configuration is
run twice under identical inputs; the only difference is attestation off (all
`UserTransactionV1`, the zero-attestation control — "A") vs on (all
`UserTransactionV2`, attested — "B"). Diff the metrics; no pass/fail threshold.

### Experiment as run

Rather than a single rate, H1 sweeps a matrix so the overhead is measured across
the whole per-transaction compute range, both client submission paths, and three
load levels. Driven by `stress-attestation-sequencer/h1/matrix.sh` (each
configuration calls `run.sh`, which runs A then B back-to-back on a fresh
genesis, scrapes Prometheus into per-run JSON, aggregates, and plots):

- **Workload**: `slow::slow(n, size)` with `n == size`, owned-object
  (`SLOW_SHARED=false`) — pure per-transaction compute, no shared object, so no
  congestion control or scheduling noise to confound the A↔B delta.
- **compute** (`slow_size`): {0, 50, 100, 200, 500} — 0 ≈ a no-op floor
  (~`gas_rounding_step`), rising per-transaction computation cost.
- **path**: `f` = submit via fullnode (`DIRECT=false`); `v` = pinned
  direct-to-one-validator (`DIRECT=true NUM_TARGET_VALIDATORS=1`).
- **rate** (`target_qps`): {200, 1000, 2000}.
- 5 × 2 × 3 = **30 configurations**, **10 iterations** each; every iteration runs
  Run A (V1, attestation OFF) and Run B (V2, attestation ON).

`run.sh` re-bootstraps a fresh genesis (empty DB) between A and B so both share
the same cold baseline and warmup — only attestation differs — while leaving the
monitoring stack (Prometheus/Grafana) up so both windows stay queryable.

Aggregation and reporting tooling (all under `h1/`):

- `make_table.py` → `results/summary_table.md` (+ `results/summary_table.csv`):
  one row per configuration, an A/B cell per metric (`mean ± std` pooled over
  time × iterations), with the network-level series computed exactly as `plot.py`
  does (rate / `histogram_quantile`, per-validator collapse).
- `summary_plot.py` → `results/summary_plots/*.png`: grouped A vs B bar charts
  per metric, configurations on the x-axis, log-scale y.

> [!NOTE]
> Client-side `settlement_finality_latency` and `submit_transaction_latency` are
> recorded only on the fullnode path, so they exist for `f` configurations only;
> the `v` (direct-to-validator) configurations bypass the fullnode and report no
> client-side latency.

### Findings (10 iterations per configuration)

Numbers below are pooled means. Per-configuration means are tight
(cross-iteration standard error ≈ 0.3–2 % for the stable metrics), so 10
iterations resolve every effect reported here; the near-parity metrics (e.g.
throughput) sit at the resolution limit, which is itself the finding. Figure
error bars are ±1 std (signal variability) by default; `summary_plot.py --disp
sem` switches them to the standard error of the mean.

**1. Attestation is a full execution dry-run — its cost tracks execution.**
`validator_attestation_latency` (B only) scales with per-transaction compute and
converges to the actual execution latency:

| slow_size | attest. lat. p50 | attest. lat. p99 | actual internal exec. p95 |
| --- | --- | --- | --- |
| 0   | 2.5 ms  | 5.0 ms  | 1.6 ms |
| 50  | 2.5 ms  | 5.4 ms  | 6.0 ms |
| 100 | 8.5 ms  | 20 ms   | 21 ms  |
| 200 | 129 ms  | 731 ms  | 198 ms |
| 500 | 944 ms  | 994 ms  | 1.28 s |

At the no-op floor (slow0) attestation costs ~2.5 ms — the fixed cost of the
dry-run machinery. As compute grows the dry-run dominates and its latency
approaches the execution cost itself: an attested transaction is executed once
for the dry-run and once for real, so heavy transactions pay roughly twice.

**2. Throughput: no penalty.** Finalized TPS
(`transactions_included_in_checkpoint`) is statistically identical A↔B — median
(B−A)/A = **+0.1 %**, within the ~0.6 % standard error. Attestation does not
reduce throughput at any compute level or rate. (The wide raw range is confined
to the slow500 configurations, where absolute throughput is small and noisy.)

**3. CPU: attestation adds ~30 % busy cores.** Per-validator CPU (busiest
validator, cadvisor) B/A median = **1.29×** (range 1.02–1.95×) — e.g.
slow100-f-q1000 8.7 → 11.1 cores, slow500-f-q1000 21.0 → 24.8 cores. Consistent
with the extra dry-run execution.

**4. Submit latency (fullnode path): a fixed per-transaction addition.** B's
submit `p50` exceeds A's by roughly the attestation latency, so the *ratio* is
largest where the baseline is smallest (low rate / low compute): slow0-q200
4.4 ms → 16.7 ms (3.8×), slow500-q200 26 ms → 693 ms (26×, i.e. +667 ms ≈ the
attestation cost). At high rate the queueing baseline dominates and the ratio
shrinks (~1.1–4×). The *added* latency is essentially the dry-run time.

**5. Internal execution latency: unchanged.**
`authority_state_internal_execution_latency` (the real, post-consensus VM
execution) is A≈B (median B/A = **1.00**, range 0.77–1.32). Attestation adds
nothing to actual execution — its cost lives entirely in the pre-consensus
dry-run.

**6. Post-consensus validation drops appear only WITHOUT attestation.**
`consensus_handler_validation_dropped_transactions` is non-zero only on A
(attestation OFF), and only at high per-transaction load: slow200-v-q1000
≈ 64/s, slow200-v-q2000 ≈ 43/s, slow200-f-q2000 ≈ 2/s, slow200-f-q1000 ≈ 0.8/s,
slow500-v-q200 ≈ 0.35/s. B (attestation ON) shows near-zero drops in every
configuration — but note these runs carry the coin-deny fix (see the H4
warning), which converts attested transactions' transient post-consensus
load-error drops into keeps. Those are the same drops that previously hit the
attested path and forked it, so B's zero is partly the fix, not an intrinsic
property of attestation. The open question is the V1 side: the unattested path
still drops under load via `handle_transaction_validation_checks` (the same
input-loading path), yet V1 does not fork. That asymmetry warrants a dedicated
follow-up.

**7. Compute-unit accounting is exact.** Attested computation units equal actual
computation units for every owned-object configuration (ratio = 1.0), confirming
attestation predicts the computation cost precisely for these transactions.

### H4 — safety (pass/fail)

**PASS.** All safety counters are zero across the pooled runs (split-brain /
remote checkpoint forks, inconsistent state hash, double-spend attempts,
attestation task panics, soft-lock equivocation), and no validator crash,
restart, or OOM occurred. The A-only validation drops in finding 6 are a
throughput/liveness observation, not a safety-counter failure.

> [!WARNING]
> This PASS holds *with a fix in place*. Earlier stress-testing hit a **checkpoint
> fork on the attested (V2) path** under load. `check_coin_deny_list_for_attested_tx`
> loads a transaction's owned inputs at their referenced versions *before*
> execution; post-consensus validation runs ahead of the execution frontier, so
> an input that is a not-yet-executed predecessor's output reads back `ObjectNotFound`.
> The code lumped that transient load error together with a real deny-list violation
> and dropped the transaction. Because the drop depends on each node's execution
> frontier it is per-node, so validators disagree on checkpoint content and hit
> `fatal!` "Local checkpoint fork detected" (`crates/iota-core/src/checkpoints/mod.rs`).
> Only the attested path forks; V1 is clean. The runs above carry a temporary
> test-branch fix that drops only on a genuine deny-list verdict
> (`CoinTypeGlobalPause` / `AddressDeniedForCoin`) and keeps the transaction on
> any load error (its inputs are present at execution, where the V1 deny-list check
> still catches global-pause / denied recipients). The fix lives on
> `protocol-research/fix/attestation-coin-deny-post-consensus-drop-fork` and is
> confirmed: the same workload that forked ~35 % of iterations ran 10/10 clean on
> EPYC (no restarts, no `exit=139`). Tracking:
> [iota-private#438](https://github.com/iotaledger/iota-private/issues/438#issuecomment-4866911507).

### Takeaway

Attestation's cost is a **pre-consensus execution dry-run**: a ~2.5 ms fixed
floor that converges to a full extra execution for heavy transactions (roughly
2× execution cost, ~+30 % validator CPU), plus a matching fixed addition to
fullnode submit latency. It does **not** cost throughput and does **not** change
actual (post-consensus) execution latency. The only surprise is finding 6 —
validation drops under load with attestation off — which needs its own
investigation.

Full per-configuration numbers: `h1/results/summary_table.md`. Figures:
`h1/results/summary_plots/*.png`.
