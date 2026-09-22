# H2 — `TotalComputationUnits` mode vs `TotalTxCount`

H2 measures the difference in throughput, latency and scheduling efficiency
between the `TotalComputationUnits` and `TotalTxCount` congestion modes (see
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
and `matrix.sh` sweeps it over a grid of cost points and limits.
Shared network scripts (`start.sh`, `cleanup.sh`, `bootstrap.sh`) live one level
up in `../`.

## Running the probe

```bash
# SLOW_N and SLOW_SIZE are required. Starts the network if none is running
# (attestation ON, TotalComputationUnits) and reuses it otherwise. Never wipes
# between invocations.
SLOW_N=100 SLOW_SIZE=100 ./probe.sh

# Sweep several points on one network.
./probe_sweep.sh                 # all 32 points
./probe_sweep.sh ladder          # the product ladder only
./probe_sweep.sh split           # the equal-product points only
./probe_sweep.sh cu              # the mode comparison's 12 cost points only
```

Each invocation prints the per-transaction result and appends a row to
`results/probe/calibration-<machine>.csv`. `<machine>` is a label of the CPU
model of the machine it ran on (for example, `ryzen-9-9950x3d` or
`epyc-9454p`), so sweeps from different machines do not collide and the
analysis scripts can distinguish them:

```text
start_epoch, slow_n, slow_size, product, shared, qps, duration, n_samples,
attested_cu, actual_cu, exec_mean_ms, exec_std_ms, exec_sem_ms,
ckpt_lag_mean_ms, ckpt_lag_p50_ms, ckpt_lag_p95_ms, ckpt_lag_p99_ms, ckpt_n,
user_txs_per_ckpt
```

The checkpoint columns are a loading check, not a result: at 5 QPS most
checkpoints hold no workload transaction, so `user_txs_per_ckpt` says whether
the point ran light enough to measure one transaction at a time.

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
for `cu1k`, 100,000 for `cu10k`, 50,000,000 for `cu5m`. Those differ by 5,000×,
which is why the limit has to be picked from a measurement rather than
converted from `LIMIT_A`.

```bash
# one cost point, one limit, 3 iterations
LABEL=cu10k-lim100k-qps1000 ITERS=3 WORKLOAD=slow SLOW_N=160 SLOW_SIZE=100 \
  LIMIT_A=10 LIMIT_B=100000 TARGET_QPS=1000 ./run.sh

# the whole grid, or one cost point / limit / the mixed configs at a time
ITERS=5 ./matrix.sh
ITERS=5 ./matrix.sh cu10k
ITERS=5 ./matrix.sh lim100k
ITERS=1 ./matrix.sh mix
```

The limit to look for is the most units a commit can admit for one object before
execution falls behind and checkpoint lag grows. How many transactions that is
depends on what they cost, where `TotalTxCount` always admits 10.

The top of the range is fixed by the protocol. A transaction is metered against
`min(gas_budget, max_gas_computation_bucket × gas_price)`, so no transaction can
be charged more than 5,000,000 computation units, whatever budget it declares
(see `probe-test.md`). Ten of those is 50,000,000 units, so that is the widest
per-object limit a 10-transaction commit could ever need, and it is the grid's
top limit.

The burst above the base limit is off by default (`OVERSHOOT_A=0`,
`OVERSHOOT_B=0`), so each run is described by one number and no debt is carried
between commits. Production runs `TotalTxCount` with an overshoot of 100 on top
of the base 10 (protocol version 22 and later), so the main grid compares the
two limits, not production's exact setting. The three `-burst` configurations
put the burst back on the limits the write-up recommends — Run A at production's
100, Run B at ten times its own base, against the same limits with the burst off
— and measure no difference: under load that keeps execution on the object
continuous the overshoot is spent once and then repaid (`RESULTS.md`, finding
7).

With the burst off, a limit below the cost of a _single_ transaction admits
nothing at all: the scheduler needs `start_time + cost <= limit` and
`start_time` is at least 0, so every transaction is deferred each commit and
then cancelled at `MAX_DEFERRAL_ROUNDS`. That is why each cost point's limits
start at or above its own per-transaction cost, and why the tightest meaningful
limit for `cu5m` is one transaction per commit.

The rate is the second knob: it sets how many transactions are available per
commit, and a limit only matters when more arrive than it admits. Every
configuration runs at 1,000 tx/s except the `-qps2000` ones: at 1,000 units the
limit already turns transactions away at that rate, and from 5,000 units up the
client's in-flight cap lowers what it offers to what can execute on the object,
so a higher target changes nothing there (`RESULTS.md`, finding 3). The
`-qps2000` configurations therefore cover only the two lightest cost points and
the mixes whose cheap cost is 1,000 units, where more arrivals can still change
the result.

Computation units are machine-independent, but execution time is not, so the
same limit fills the object differently on each machine — measure where
checkpoint lag starts growing on that machine rather than reusing a number from
elsewhere.

Both scripts submit through the fullnode (`DIRECT=false`, as in H1): one mutable
shared object caps throughput low enough that these rates should stay under what
the fullnode can push, and that path keeps the client's latency in Prometheus.
`DIRECT=true` switches to a client in docker submitting straight to the
validators, and its throughput and latency then come only from the report it
prints (`run-*-stress-report.log`), which every run saves either way. The one
configuration that may need it is `cu1k`, where the drain rate is in the
thousands of transactions a second.

The grid uses `slow` (W5) throughout. It publishes one `slow::Obj` and every
transaction takes it as a mutable input, so all of them contend on the same
object; the workload has no setting for more objects.

The plan's W1 (`shared`, `--shared-counter`) is not in the grid. With
`NUM_SHARED_COUNTERS=1` every transaction increments the same counter at a cost
that also lands on the 1,000-unit floor, which is what the `cu1k` configurations
already run — same one hot object, same uniform cost. `run.sh` still takes
`WORKLOAD=shared`, so it is available as an independent workload to cross-check
against if the `slow` numbers look surprising.

### Mixed cost, and why the fixed-cost grid cannot separate the modes

With one fixed cost per transaction the two modes are the same scheduler: if
every transaction costs C, a unit limit L admits `L / C` of them, which is
exactly what a count limit of `L / C` admits. The grid measures that — across
its twelve matched configurations, spanning a 5000× cost range, Run B lands
within 1.6% of Run A on throughput and latency alike.

The modes can only differ when transactions in ONE commit cost different
amounts. Then a count limit admits a fixed number and lets the admitted work
swing with the mix, while a unit limit admits a fixed amount of work and lets
the number swing instead. `SLOW_MIX` draws each transaction's `slow_n` from a
weighted list, so a commit carries a spread:

```bash
# 9 transactions of 1,000 units for every 1 of 100,000 (mean 10,900), against
# a limit of ten mean-cost transactions
LABEL=mix10900-w10-lim109k-qps1000 ITERS=1 WORKLOAD=slow \
  SLOW_MIX=1:9,350:1 SLOW_SIZE=100 LIMIT_A=10 LIMIT_B=109000 \
  TARGET_QPS=1000 ./run.sh
```

Every cost shares `SLOW_SIZE`, so the mix varies `n` alone, and each one
costs whatever the calibration measured for that `n`. `SLOW_MIX` overrides
`SLOW_N`. `run.sh` refuses to start on a malformed spec, and on `SLOW_SIZE=0`,
where `slow::slow(n, 0)` writes n EMPTY vectors so every cost would land
onto the cost floor and the spread would vanish unnoticed.

The usable weights for the expensive transactions run from 10% to about 40%:

- Below 10%, `LIMIT_B = 10 × mean` falls below one expensive transaction's own
  cost, so Run B could never schedule one at all — it would cancel every one
  of them, which is a different experiment.
- Above about 40% the expensive transactions alone keep execution on the object
  saturated, so the count limit leaves no idle capacity for the unit limit to
  use: at 30% the gain is 1.4×, at 50% (`mix50500`) 1.3× with checkpoint lag
  above 15 s in both runs.

Each mixed configuration is compared with the `cu` configuration of the same
mean cost, already run: same mean cost, same mean admitted work, uniform against
spread.

Results follow the H1 layout: `results/matrix/<LABEL>/iter-NNN/`, one
configuration per label, enforced by the same `config.json` check
(`../exp_dir.py`). The probe's own outputs are kept apart, under
`results/probe/`:

```text
results/matrix/<LABEL>/
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
  Run B. Needs `LABEL` and `LIMIT_B`. `SLOW_N` gives every transaction one
  cost; `SLOW_MIX` draws a cost per transaction so a commit carries a spread,
  which is the only setting in which the two modes differ.
- `matrix.sh` — runs `run.sh` over the configuration grid, one iteration of
  every configuration per round, `ITERS` rounds, with one log per configuration
  under `logs/`.
- `aggregate.py` — pools every label's iterations into one A-vs-B table per mode
  pair (`results/matrix/summary.md`): success tps (executed − cancelled −
  commits, the user transactions that did real work), the finalized
  checkpoint-inclusion rate, cancelled rate, checkpoint lag (the exact histogram
  mean and the exact share over 30s, plus the pooled p95), skipped leader
  rounds, and the safety verdict (counters + validator crash scan); then the
  spread of success tps, cancellations and checkpoint lag across the iterations
  (sample standard deviation), and for the mixed-cost configurations the rate of
  transactions executed at the expensive cost. The same rows land as scalars in
  `results/matrix/summary.csv` for `plot.py`, with each configuration's rate,
  run duration and overshoot alongside them, the per-commit admission histogram
  of every run in `results/matrix/admits_hist.csv`, and the checkpoint lag per
  10 s and 60 s slice of the window in `results/matrix/lag_over_time.csv`.
  Standard library only; the code shared with `../h1/aggregate.py` lives in
  `../aggregate.py`.
- `plot.py` — renders the mode-comparison figures from `aggregate.py`'s outputs
  into `results/matrix/summary_plots/`. Over the fixed-cost configurations:
  checkpoint lag and cancelled fraction against the admitted rate (tx/commit ×
  commits/s, with Run A as one vertical line; `modes_admitted_rate.png`), the
  same against admitted rate divided by the drain rate
  (`modes_utilization.png`), annotated per-configuration heatmaps
  (`modes_heatmaps.png`), and Run A next to Run B at the twelve matched
  configurations with the spread across iterations (`modes_matched.png`). Over
  the mixed-cost configurations: `modes_mix.png` for the mixes at `LIMIT_B = 10
  × mean` — how many transactions each commit admitted, Run A next to Run B,
  with the throughput, cancellation and checkpoint lag outcome below — and
  `modes_mix_ladders.png` for the mixes run at several limits: success tps,
  cancellations, checkpoint lag and the expensive transactions executed per
  second against `LIMIT_B`, with Run A as the reference and error bars from the
  iterations. `modes_lag_over_time.png` draws the checkpoint lag per slice of
  any run longer than the usual 60 s. With a second results directory as the
  second argument (`plot.py results/matrix results/matrix-ws`) it also draws the
  ladders both machines ran, in `modes_two_machines.png`. Needs matplotlib, so
  run it from a `venv` such as `../h1/.venv`.

  The figures cover one set of configurations at a time: one target rate, one
  run duration, and the burst off. A configuration that varies any of those
  would otherwise land on top of a baseline configuration at the same limit, so
  it is left out and read from `summary.md` instead; the run prints how many
  were skipped. `QPS=2000 ./plot.py results/matrix` draws that rate instead,
  into file names carrying a `-qps2000` suffix so the default figures stay put.
- `probe.sh` — run one `(SLOW_N, SLOW_SIZE)` point: start the network or reuse a
  running one, scrape metrics, append a CSV row, and optionally tear down
  the network (by default, it leaves the network up).
- `probe_scrape.py` — reads Prometheus and computes the statistics. Standard
  library only, so it needs no `venv`.
- `probe_sweep.sh` — runs `probe.sh` over several points on one network.
- `compare_machines.py` — joins two `calibration-<machine>.csv` files and
  prints a table comparing them: whether the computation units match, and
  what the execution-time ratio is. Standard library only, and it writes no
  files.
- `plot_calibration.py` — renders the calibration figures into
  `results/probe/`. Needs matplotlib, so run it from a `venv` such as
  `../h1/.venv`.

The calibration is written up in `probe-test.md`; the mode comparison in
`RESULTS.md`.

## Next steps

- **Under overload, both modes admit fewer transactions than their limit
  allows.** From 5,000 units up, Run A admits 7.4 down to 3.8 transactions per
  commit against a limit of 10 while dozens sit deferred, and the share of
  commits that schedule anything falls to 26 % at the heaviest point. Both
  modes do it equally, so it does not affect the comparison, but the scheduler
  is leaving capacity unused while transactions wait to be cancelled. Debt
  carried between commits is ruled out (overshoot 0) and the suggested-gas-price
  code is advisory on this path; why it happens is not yet known
  (`RESULTS.md`, finding 3).
- **The deferral limit is counted in leader rounds.** A skipped leader round
  uses up a round without a scheduling attempt, so 2–4 % of deferrals are
  cancelled after 11–12 rounds instead of 10. Small here, but the limit is
  not what it says; worth an upstream issue proposing to count evaluations.
- **Checkpoint lag over time for more configurations.** A pooled checkpoint lag
  statistic cannot distinguish a queue that is high but stable from one growing
  without bound; the checkpoint lag slices cover the four 300 s runs
  (`modes_lag_over_time.png`), and the 60 s configurations only as numbers in
  `lag_over_time.csv`. Those four showed the distinction matters — two
  configurations that look mild over 60 s climb for a whole 300 s run — so the
  limits just past the drain rate are worth the same treatment.
  `consensus_handler_transaction_deferral_rounds` is also still unplotted.
- **The overshoot against bursty arrivals.** The three `-burst` configurations
  answered the question they were added for: with production's overshoot on,
  nothing changes, because load that keeps execution on the object
  continuous spends the overshoot once and then repays it (`RESULTS.md`, finding
  7). That leaves the case the overshoot exists for untested — arrivals that are
  bursty rather than saturating. The client here offers a steady rate, so this
  needs a workload that idles the object and then floods it.
- **A transaction that overruns the commit on its own.** A 500,000-unit
  transaction executes for ≈80 ms against a ≈50 ms commit on EPYC, so no unit
  limit serves `mix50900` there: admitting one per commit builds checkpoint lag,
  excluding it never lets those transactions through. On the WS the same
  transaction is 17.7 ms, two fit, and 1M is a good limit — so the problem is
  the machine, not them. Whether traffic whose single transaction overruns the
  commit on the hardware in question should be admitted at all, and how, is a
  design question the data cannot settle.
- **Where the ladders peak above 1,000 tx/s.** At double the submission rate
  several configurations turn out to have been limited by the client rather than
  by the limit, and on the WS the `mix3700` ladder's success tps equals the
  offered load at its top limit. Both mean the peak of those ladders is above
  what was measured. Finding it needs a higher target rate, or more in-flight
  transactions, on the limits that showed no cancellations.

