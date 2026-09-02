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
distinguish them:

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

`LIMIT_A` is a transaction count per object per commit, `LIMIT_B` computation
units per object per commit; neither is computed from the other. `LIMIT_A`
defaults to production's 10, `LIMIT_B` is required. Ten transactions of a
workload costing C units each is `10 × C` units of work, so `LIMIT_B = 10 × C`
is the limit that admits the same work as Run A at that one cost: 10,000 units
for `cu1k`, 160,000 for `cu16k`, 4,910,000 for `cu491k`. Those differ by 491×,
which is why the limit has to be picked from a measurement rather than
converted from `LIMIT_A`.

```bash
# one cost point, one limit, one rate, 3 iterations
LABEL=cu16k-lim160k-qps500 ITERS=3 WORKLOAD=slow SLOW_N=200 SLOW_SIZE=100 \
  LIMIT_A=10 LIMIT_B=160000 TARGET_QPS=500 ./run.sh

# the whole grid, or one cost point / limit / rate at a time
ITERS=5 ./matrix.sh
ITERS=5 ./matrix.sh cu491k
ITERS=5 ./matrix.sh lim160k
ITERS=5 ./matrix.sh qps2000
```

The limit to look for is the most units a commit can admit for one object before
execution falls behind and checkpoint lag grows. How many transactions that is
depends on what they cost, where `TotalTxCount` always admits 10.

The top of the range is fixed by the protocol. A transaction is metered against
`min(gas_budget, max_gas_computation_bucket × gas_price)`, so no transaction can
be charged more than 5,000,000 computation units, whatever budget it declares
(see `probe-test.md`). Ten of those is 50,000,000 units, so that is the widest
per-object limit a 10-transaction commit could ever need, and it is the grid's
top rung.

The burst above the base limit is off by default (`OVERSHOOT_A=0`,
`OVERSHOOT_B=0`), so each run is described by one number and no debt is carried
between commits. Once a limit is settled, re-run it with an overshoot ten times
the base to see what the burst adds.

With the burst off, a limit below the cost of a _single_ transaction admits
nothing at all: the scheduler needs `start_time + cost <= limit` and
`start_time` is at least 0, so every transaction is deferred each commit and
then cancelled at `MAX_DEFERRAL_ROUNDS`. That is why each cost point's limits
start at or above its own per-transaction cost, and why the tightest meaningful
limit for `cu491k` is one transaction per commit.

The rate is the second knob: it sets how many transactions are available per
commit, and a limit only binds when demand exceeds what it admits, so each cell
pairs a limit with a rate high enough to saturate it. The limits that match Run
A's capacity run the whole 250/500/1000/2000 ladder.

Computation units are machine-independent, but execution time is not, so the
same limit saturates differently on each machine — measure where lag starts
growing on that machine rather than reusing a number from elsewhere.

Both scripts submit through the fullnode (`DIRECT=false`, as in H1): one
mutable shared object caps throughput low enough that these rates should stay
under what the fullnode can push, and that path keeps the client's latency in
Prometheus. `DIRECT=true` switches to a client in docker submitting straight
to the validators, and its throughput and latency then come only from the
report it prints (`run-*-stress-report.log`), which every run saves either
way. The one cell that may need it is `cu1k`, whose object can drain thousands
of transactions a second.

The grid uses `slow` (W5) throughout. It publishes one `slow::Obj` and every
transaction takes it as a mutable input, so all of them contend on the same
object; the workload has no setting for more objects.

The plan's W1 (`shared`, `--shared-counter`) is not in the grid. With
`NUM_SHARED_COUNTERS=1` every transaction increments the same counter at a cost
that also lands on the 1,000-unit floor, which is what the `cu1k` cells already
run — same one hot object, same uniform cost. `run.sh` still takes
`WORKLOAD=shared`, so it is available as an independent workload to cross-check
against if the `slow` numbers look surprising.

Results follow the H1 layout: `results/<LABEL>/iter-NNN/`, one config per
label, enforced by the same config gate (`../exp_dir.py`):

```text
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
  Run B. Needs `LABEL` and `LIMIT_B`.
- `matrix.sh` — runs `run.sh` over the config grid, one iteration of every
  config per round, `ITERS` rounds, with one log per config under `logs/`.
- `aggregate.py` — pools every label's iterations into one A-vs-B table per
  mode pair (`results/summary.md`): success tps (executed − cancelled −
  commits, the user transactions that did real work), the finalized
  checkpoint-inclusion rate, cancelled rate, checkpoint lag (the exact
  histogram mean and the exact share over 30s, plus the pooled p95),
  skipped leader rounds, and the safety verdict (counters +
  validator crash scan). The same rows land as scalars in
  `results/summary.csv` for `plot.py`. Standard library only; the machinery
  shared with `../h1/aggregate.py` lives in `../aggregate.py`.
- `plot.py` — renders the mode-comparison figures from `summary.csv` into
  `results/summary_plots/`: checkpoint lag and cancelled fraction against the
  admitted rate (tx/commit × commits/s, with Run A as one vertical line),
  annotated per-cell heatmaps of the same scalars, the throughput-vs-lag
  tradeoff, and lag against admitted/drain utilization.
  Needs matplotlib, so run it from a `venv` such as `../h1/.venv`.
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

- **Re-measure the shared-object execution times.** The shared-object probe
  runs measured the attested units (they are in `matrix.sh`'s cost table) but
  not the execution times: the probe started its network with the default
  per-object limit of 10 units, below any transaction's cost, so every
  shared-input transaction was deferred and cancelled instead of executed.
  `probe.sh` needs to start the network with a limit no probe transaction can
  reach (e.g. `MAX_ACCUMULATED_TXN_COST=50000000`), then the sweep re-run and
  the `drains` column in `matrix.sh` filled in.
- **Per-cell time series for the marginal cells.** A pooled lag statistic
  cannot distinguish a queue that is high but stable from one growing
  without bound — a lag-over-time curve can. Worth adapting `../h1/plot.py`'s
  dashboard replay as a drill-down for a few chosen cells (the knife-edge
  ones), not for the whole grid. Also still unplotted:
  `consensus_handler_transaction_deferral_rounds` and
  `consensus_handler_scheduled_transactions_per_object_per_commit`.
- **Add a run whose transactions do not all cost the same.** The grid is all
  fixed-cost, which is the control; the modes can only differ when the cost
  varies, since that is when a count limit and a cost limit admit different
  amounts of work. The only mixed-cost workload available today is
  `slow::bimodal`, which alternates every 10s between 4,000 and 1,000
  computation units — a factor of 4, with both levels fixed in the Move code,
  and still uniform within any one commit. A useful version needs either
  configurable levels in `slow::bimodal` or two `slow` weights with different
  `n` running at the same time (benchmark groups run one after another, so they
  cannot serve this today).
