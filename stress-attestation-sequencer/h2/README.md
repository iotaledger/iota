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
  CU_PER_TX=4000 LIMIT_A=10 TARGET_QPS=500 ./run.sh

# Run B's limits set directly instead of computed, for a workload with no
# single per-transaction cost
LABEL=mixed-lim20k-qps500-n4 WORKLOAD=slow LIMIT_B=20000 \
  OVERSHOOT_B=0 TARGET_QPS=500 ./run.sh

# the whole grid, 5 iterations each, or one cost point at a time
ITERS=5 ./matrix.sh
ITERS=5 ./matrix.sh cu4k-
```

Every config in the grid runs one fixed cost, so all the transactions in a run
are identical and both modes admit the same work once the limits match. That
makes the grid the control: it shows whether `TotalComputationUnits` keeps up
with `TotalTxCount` at each gas bucket. A run whose transactions differ in cost
— where the two modes would admit different amounts of work — comes later; see
Next steps.

The burst above the base limit is off by default (`OVERSHOOT_A=0`, and Run B's
follows from it), so each run is described by one number: `LIMIT_A`, the
transaction count per object per commit that Run B's limit is computed from.
With the burst off nothing exceeds the base limit and no debt is carried into
later commits, so the two runs differ only in that one number — with a burst,
the debt would be carried in transactions on one side and in computation units
on the other. Once a base limit is settled, re-run it with
`OVERSHOOT_A=$((10 * LIMIT_A))` to see what the burst adds.

`run.sh` defaults `LIMIT_A` to production's 10, but 10 transactions per object
per commit may well be below what four validators can execute, in which case
the limit and not the mode is what caps throughput. `matrix.sh` therefore
spells it out in every cell and runs the two lightest points at 100 as well.

One constraint comes with the burst off: the base limit still has to fit a
single transaction, or that transaction is deferred every commit and cancelled
after `MAX_DEFERRAL_ROUNDS`. `LIMIT_A >= 1` covers `TotalTxCount`, and
`LIMIT_A × CU_PER_TX` leaves `LIMIT_A` transactions of headroom under
`TotalComputationUnits`, so this only bites if the real attested cost is far
above `CU_PER_TX` — which is the other reason to measure it first.

Both scripts submit through the fullnode (`DIRECT=false`, as in H1): one
mutable shared object caps throughput low enough that these rates stay under
what the fullnode can push, and that path keeps the client's latency in
Prometheus. `DIRECT=true` switches to a client in docker submitting straight
to the validators, and its throughput and latency then come only from the
report it prints (`run-*-stress-report.log`), which every run saves either
way.

The grid uses `slow` (W5) throughout. It publishes one `slow::Obj` shared
object and every transaction takes it as a mutable input, so all of them
contend on the same object; the workload has no setting for more objects.
Transactions on one mutable shared object also execute one after another, so
`matrix.sh` picks the rates per cost point rather than using the same rates
everywhere.

The plan's W1 (`shared`, `--shared-counter`) is not in the grid. With
`NUM_SHARED_COUNTERS=1` every transaction increments the same counter at a cost
that also lands on the 1,000-unit floor, which is what the `cu1k` cells already
run — same one hot object, same uniform cost. `run.sh` still takes
`WORKLOAD=shared`, so it is available as an independent workload to cross-check
against if the `slow` numbers look surprising.

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
  the comparison no longer measures the mode.
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
- **Add a run whose transactions do not all cost the same.** The grid is all
  fixed-cost, which is the control; the modes can only pull apart when the cost
  varies, since that is when a count limit and a cost limit admit different
  amounts of work. The only mixed-cost workload available today is
  `slow::bimodal`, which alternates every 10s between 4,000 and 1,000
  computation units — a factor of 4, with both levels fixed in the Move code,
  and still uniform within any one commit. A useful version needs either
  configurable levels in `slow::bimodal` or two `slow` weights with different
  `n` running at the same time (benchmark groups run one after another, so they
  cannot serve this today).
