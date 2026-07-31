# H2 — `TotalComputationUnits` mode vs `TotalTxCount`

H2 measures the throughput, latency, scheduling efficiency difference between
the `TotalComputationUnits` and `TotalTxCount` congestion modes (see
`../stress-plan.md`). For the comparison to be fair, each mode's per-object
limit has to let the same amount of work through. Converting one limit into
the other needs the attested computation units per transaction for the
workload:

```text
limit_CU = limit_tx_count × (attested computation units per transaction)
```

Measuring that number is the first step, and `probe.sh` does it: it runs
`slow::slow(n, size)` and records the computation units and execution time it
produces. The probe uses the owned-object form of the workload (W4 in
`../stress-plan.md`). Its output picks the `(n, size)` settings for the mode
comparison, which uses the shared-object form (W5), and sets the limits.
`run.sh` then runs the comparison itself — one run per mode on the same load —
and `matrix.sh` sweeps it over a grid of cost points and rates.
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

```text
start_epoch, slow_n, slow_size, product, shared, qps, duration, n_samples,
attested_cu, actual_cu, exec_mean_ms, exec_std_ms, exec_sem_ms
```

### What it measures

- **Computation units** — `mean = Δ_sum / Δ_count` of
  `attested_computation_units` and `actual_computation_units`. The workload is
  deterministic, so this mean is the exact per-transaction value. For the
  probe's owned-object transactions, the two should be equal, because no state
  can change between the attestation dry-run and execution. With shared
  objects, it can change, which is untested so far. This is the number the
  limits are computed from.
- **Execution time** — `authority_state_internal_execution_latency_user`, which
  covers only post-consensus VM execution of user transactions, pooled across
  the validators and excluding the fullnode's checkpoint-replay executions.
  Reported as `mean ± sem`, with `std` (from histogram bucket deltas) and the
  sample count `N`. The probe runs at a low rate so nothing queues, which
  makes this the per-transaction cost on an idle network. The `_user` histogram
  is used because the all-transactions one also counts the network's steady
  stream of system transactions (commit prologues and similar), which outnumber
  the probe's transactions roughly 30 to 1 and pull the mean down toward their
  sub-millisecond cost.

### Why the points are spaced geometrically

Computation units are rounded up to a multiple of `gas_rounding_step` (1000),
and they grow much faster than the product `n·size` — in H1, raising the
product 4× raised computation units about 40×. So the ladder points step the
product geometrically (`size` fixed at 100, varying `n`), which spreads them
evenly once the units are on a log scale and puts points either side of each
rounding step. `slow::slow` writes about `n·size` vector elements, so the
product is what drives the cost. The `split` points hold the product at 40000
while changing how it divides between n and size, which checks that only the
product matters.

## Running the mode comparison

`run.sh` is `../h1/run.sh` with the two runs changed from attestation off/on
to one congestion mode each. Per iteration it bootstraps a 4-validator network
and runs the same load twice: Run A in `MODE_A` (default `TotalTxCount`) and
Run B in `MODE_B` (default `TotalComputationUnits`), saving each run's raw
Prometheus window. Attestation is on in both runs, so the mode is the only
thing that differs; with it off, `TotalComputationUnits` has no attested cost
to schedule on and falls back to `gas_budget / gas_price`.

Both runs have to be able to admit the same amount of work, which means
different numeric limits. Set `CU_PER_TX` to the workload's attested
computation units per transaction and Run B's limits are computed from Run
A's:

```bash
# one point, one rate, 3 iterations
LABEL=cu4k-qps500-n4 ITERS=3 WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 \
  CU_PER_TX=4000 LIMIT_A=10 OVERSHOOT_A=100 TARGET_QPS=500 ./run.sh

# limits set directly, for the bimodal workload: its transactions do not all
# cost the same, so there is no single number to compute them from
LABEL=bimodal-lim20k-qps500-n4 WORKLOAD=slow LIMIT_B=20000 \
  OVERSHOOT_B=200000 TARGET_QPS=500 RUN_DURATION=120s ./run.sh

# the whole grid, 5 iterations each, one group at a time
ITERS=5 ./matrix.sh cu       # one fixed cost per run
ITERS=5 ./matrix.sh bimodal  # heavy and light transactions alternating
ITERS=5 ./matrix.sh counter  # shared-counter baseline (W1)
```

`LIMIT_A` and `OVERSHOOT_A` are the transaction count per object per commit
that Run B's limits are computed from. `run.sh` defaults them to the
production 10 and 100, but 10 transactions per object per commit may well be
below what four validators can execute, in which case the limit and not the
mode is what caps throughput. `matrix.sh` therefore spells the pair out in
every cell, and runs the two lightest points at 100 and 1000 as well.

Both scripts submit through the fullnode (`DIRECT=false`, as in H1): one
mutable shared object caps throughput low enough that these rates stay under
what the fullnode can push, and that path keeps the client's latency in
Prometheus. `DIRECT=true` switches to a client in docker submitting straight
to the validators, and its throughput and latency then come only from the
report it prints (`run-*-stress-report.log`), which every run saves either
way.

The workloads are the two shared-object ones the plan uses: `slow` (W5) and
`shared` (W1, `--shared-counter`). `slow` publishes one `slow::Obj` shared
object and every transaction takes it as a mutable input, so all of them
contend on the same object; the workload has no setting for more objects.
Transactions on one mutable shared object also execute one after another, so
`matrix.sh` picks the rates per cost point rather than using the same rates
everywhere.

Results follow the H1 layout: `results/<LABEL>/iter-NNN/`, one config per
label, enforced by the same config gate (`../exp_dir.py`):

```
results/<LABEL>/
    config.json                    # canonical inputs; rejects a changed config
    iter-001/
        run-a-timeseries.json      run-b-timeseries.json
        run-a-stress-report.log    run-b-stress-report.log
        run-a-stress.log           run-b-stress.log
        run-a-node-logs/           run-b-node-logs/
        cleanup.log  bootstrap.log
    iter-002/  ...
```

## Tooling

- `run.sh` — the mode comparison; one iteration is bootstrap, Run A, reset,
  Run B. Needs `LABEL`, and either `CU_PER_TX` or both `LIMIT_B` and
  `OVERSHOOT_B`.
- `matrix.sh` — runs `run.sh` over the config grid, one iteration of every
  config per round, `ITERS` rounds, with one log per config under `logs/`.
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

The grid in `matrix.sh` is set up but has not been run yet. Still to do:

- **Measure the computation units of the shared-object transactions.** The
  calibration measured owned-object ones; the transactions in the comparison
  carry a mutable shared input as well. Run
  `SLOW_N=<n> SLOW_SIZE=100 SLOW_SHARED=true ./probe.sh`
  for each of the five points and correct `CU_PER_TX` in `matrix.sh` if the
  numbers differ. A wrong value gives the two runs different capacity, and then
  the comparison no longer measures the mode. The same applies to the
  shared-counter group, whose `CU_PER_TX=1000` is an assumption (a counter
  increment landing on the rounding floor), not a measurement.
- **Decide what to collect and write the aggregation.** Every run saves the full
  metric set, but nothing reads it yet: H2 needs its own `aggregate.py` and
  `plot.py`, adapted from `../h1/` for two modes instead of attestation off/on.
  The numbers the plan asks for are throughput
  (`transactions_included_in_checkpoint`), latency, and how much each mode
  deferred or cancelled per object
  (`consensus_handler_deferred_transactions`,
  `consensus_handler_cancelled_transactions`,
  `consensus_handler_transaction_deferral_rounds`,
  `consensus_handler_scheduled_transactions_per_object_per_commit`).
- **Spread the transaction costs further apart.** `slow::bimodal` alternates
  between 4,000 and 1,000 computation units, a factor of 4, and its two levels
  are fixed in the Move code. Telling the modes apart clearly needs a wider
  spread: either configurable levels in `slow::bimodal`, or two `slow` weights
  with different `n` running at the same time (benchmark groups run one after
  another, so they cannot serve this today).
