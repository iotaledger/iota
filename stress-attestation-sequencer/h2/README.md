# H2 — `TotalComputationUnits` mode vs `TotalTxCount`

H2 measures the throughput, latency, scheduling efficiency difference between
the `TotalComputationUnits` and `TotalTxCount` congestion modes (see
`../stress-plan.md`). For the comparison to be fair, each mode's per-object limit
has to let the same amount of work through. Converting one limit into the other
needs the attested computation units per transaction for the workload:

```
limit_CU = limit_tx_count × (attested computation units per transaction)
```

Measuring that number is the first step, and it is all this directory does so
far: a probe that runs `slow::slow(n, size)` and records the computation units and
execution time it produces. The probe uses the owned-object form of the workload
(W4 in `../stress-plan.md`). Its output picks the `(n, size)` settings for the
mode comparison, which uses the shared-object form (W5), and sets the limits.
Shared network scripts (`start.sh`, `cleanup.sh`, `bootstrap.sh`) live one level
up in `../`.

## Running the probe

```bash
# SLOW_N and SLOW_SIZE are required. Starts the network if none is running
# (attestation ON, TotalComputationUnits) and reuses it otherwise. Never wipes
# between invocations.
SLOW_N=100 SLOW_SIZE=100 ./probe.sh

# Sweep several points on one network.
./probe_sweep.sh                 # all points
./probe_sweep.sh ladder          # the product ladder only
./probe_sweep.sh split           # the equal-product points only
```

Each invocation prints the per-transaction result and appends a row to
`results/calibration-<machine>.csv`. `<machine>` is a label of the CPU model
of the machine it ran on (for example, `ryzen-9-9950x3d` or `epyc-9454p`),
so sweeps from different machines do not collide and the analysis scripts can
tell them apart:

```
start_epoch, slow_n, slow_size, product, shared, qps, duration, n_samples,
attested_cu, actual_cu, exec_mean_ms, exec_std_ms, exec_sem_ms
```

### What it measures

- **Computation units** — `mean = Δ_sum / Δ_count` of
  `attested_computation_units` and `actual_computation_units`. The workload is
  deterministic, so this mean is the exact per-transaction value. For the probe's
  owned-object transactions, the two should be equal, because no state can change
  between the attestation dry-run and execution. With shared objects, it can
  change, which is untested so far. This is the number the limits are computed
  from.
- **Execution time** — `authority_state_internal_execution_latency_user`, which
  covers only post-consensus VM execution of user transactions, pooled across the
  validators and excluding the fullnode's checkpoint-replay executions. Reported
  as `mean ± sem`, with `std` (from histogram bucket deltas) and the sample count
  `N`. The probe runs at a low rate so nothing queues, which makes this the
  per-transaction cost on an idle network. The `_user` histogram is used because
  the all-transactions one also counts the network's steady stream of system
  transactions (commit prologues and similar), which outnumber the probe's
  transactions roughly 30 to 1 and pull the mean down toward their
  sub-millisecond cost.

### Why the points are spaced geometrically

Computation units are rounded up to a multiple of `gas_rounding_step` (1000), and
they grow much faster than the product `n·size` — in H1, raising the product 4×
raised computation units about 40×. So the ladder points step the product
geometrically (`size` fixed at 100, varying `n`), which spreads them evenly
once the units are on a log scale and puts points either side of each rounding
step. `slow::slow` writes about `n·size` vector elements, so the product is
what drives the cost. The `split` points hold the product at 40000 while
changing how it divides between n and size, which checks that only the product
matters.

## Tooling

- `probe.sh` — run one `(SLOW_N, SLOW_SIZE)` point: start the network or reuse a
  running one, scrape metrics, append a CSV row, and optionally tear down
  the network (by default, it leaves the network up).
- `probe_scrape.py` — reads Prometheus and computes the statistics. Standard
  library only, so it needs no `venv`.
- `probe_sweep.sh` — runs `probe.sh` over several points on one network.
- `compare_machines.py` — joins two `calibration-<machine>.csv` files and
  prints a table comparing them: whether the computation units agree, and
  what the execution-time ratio is. Standard library only, and it writes no
  files.
- `plot_calibration.py` — renders the calibration figures into
  `results/summary_plots/`. Needs matplotlib, so run it from a `venv` such as
  `../h1/.venv`.

The results so far are written up in `probe-test.md`.

## Next steps

Once there is enough calibration data: pick four or five slow points that land
in different gas buckets, work out each mode's limit from them, and run the
mode comparison — `TotalTxCount` against `TotalComputationUnits`, attestation
on in both — against the shared-object workloads W1 (`shared-counter`) and W5
(`slow --slow-shared true`). That comparison still needs its own `run.sh`, adapted
from `../h1/run.sh`.
