# H2 — new mode vs `TotalTxCount`

H2 measures the throughput/latency difference between the
`TotalComputationUnits` and `TotalTxCount` congestion modes (see
`../stress-plan.md`). A fair comparison needs each mode's per-object limit set
to the **same effective capacity**, which requires knowing the **attested
computation units per transaction** for the workload:

```
# same effective capacity in each mode:
limit_CU = limit_txcount × (attested CU per transaction)
```

This directory currently holds the **calibration pre-step**: a probe that maps
`slow::slow(n, size)` → (computation units, execution time). Its output selects
the W5 cost points and sets those limits. Shared network scripts (`start.sh`,
`cleanup.sh`, `bootstrap.sh`) live one level up in `../`.

## Calibration pre-step

```bash
# One point. SLOW_N and SLOW_SIZE are required. Brings the network up if none is
# running (attestation ON, TotalComputationUnits), reuses it otherwise, and does
# NOT wipe between invocations.
SLOW_N=100 SLOW_SIZE=100 ./probe.sh

# Sweep the whole grid on one network (ladder + split-invariance check).
./probe_sweep.sh                 # ladder + split
./probe_sweep.sh ladder          # ladder only
```

Each invocation prints the per-transaction result and appends a row to
`results/calibration-<machine>.csv`, where `<machine>` is a slug of the CPU
model of the box it ran on (e.g. `ryzen-9-9950x3d`, `epyc-9454p`) so sweeps from
different machines don't collide and the analysis scripts can tell them apart:

```
start_epoch, slow_n, slow_size, product, shared, qps, duration, n_samples,
attested_cu, actual_cu, exec_mean_ms, exec_std_ms, exec_sem_ms
```

### What it measures

- **Computation units** — `mean = Δ_sum / Δ_count` of
  `attested_computation_units` and `actual_computation_units`. The workload is
  deterministic, so this mean is the exact per-transaction value; the two should
  match (attestation predicts the cost exactly here). This is the number the
  mode calibration needs.
- **Internal execution time** — `authority_state_internal_execution_latency_user`
  (pure post-consensus VM execution, user transactions only): `mean ± sem`, plus
  `std` (from histogram bucket deltas) and sample count `N`. Low rate ⇒ no
  queueing ⇒ this is the intrinsic unloaded per-transaction cost. The `_user`
  histogram exists because the all-transactions one pools the network's constant
  stream of system transactions (commit prologues etc.), which outnumber the
  spam ~30:1 and drag the mean toward their sub-ms cost.

### Why a geometric grid

Computation units are quantized into `gas_rounding_step` (1000-unit) buckets and
are strongly superlinear in the product `n·size` (H1: a 4× product bump moved
CUs ~40×). `probe_sweep.sh` uses a log ladder of the product (size fixed at 100,
varying n) so the points are spaced evenly in log-CU and straddle every bucket.
`slow::slow` does `≈ n·size` push-backs, so the product is the cost axis; the
split-invariance points (equal product, different n/size) confirm that.

## Tooling

- `probe.sh` — run one `(SLOW_N, SLOW_SIZE)` point; reuse-or-start the network;
  scrape; append a CSV row; optional teardown (default: leave up).
- `probe_scrape.py` — stdlib Prometheus reader + statistics (no venv).
- `probe_sweep.sh` — loop `probe.sh` over the calibration grid on one network.
- `compare_machines.py` — join two `calibration-<machine>.csv` files and print a
  cross-machine table (CU-match check + exec-time ratio); stdlib, prints only.
- `plot_calibration.py` — render the calibration figures to
  `results/summary_plots/` (needs a matplotlib venv, e.g. `../h1/.venv`).

Results are written up in `probe-test.md`.

## Next (deferred until calibration data exists)

Pick ~4–5 slow points in distinct gas buckets from `calibration-<machine>.csv`,
set the per-mode limits, then run the mode comparison (`TotalTxCount` vs
`TotalComputationUnits`, both attestation ON) on shared-object W1
(`shared-counter`) and W5 (`slow --slow-shared true`). Harness `run.sh` to be
adapted from `../h1/run.sh`.
