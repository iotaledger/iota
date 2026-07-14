# Stress-test runs and results

Running log of the stress tests from `stress-plan.md`: the exact commands,
the results of each run, and a brief analysis.

All commands are run from the `iota` monorepo root unless noted.

---

## H1 — attestation overhead (W4: slow owned-object; V1 vs V2)

**Goal:** measure what validator attestation costs. Attestation (the
pre-consensus dry-run) happens in the `submit_tx` path independent of the
congestion mode, so H1 deliberately keeps sequencing out of the picture:
**owned-object** transactions only (no shared-object scheduling at all). Each
configuration is run twice under identical load; the only difference is
attestation off (all `UserTransactionV1`, the zero-attestation control — "A")
vs on (all `UserTransactionV2`, attested — "B"). We then compare the two runs to
see how the metrics differ between them; this is purely a measurement, with no
pass/fail threshold.

---

### Experiment as run

Rather than a single submission rate (target QPS), H1 sweeps a matrix so the
overhead is measured across a range of per-transaction computation units,
three client submission paths (via fullnode, direct to a single validator,
direct to all four), and three load levels — target QPS of 200, 1000, and
2000 tx/s. Driven by
`stress-attestation-sequencer/h1/matrix.sh` (each configuration calls `run.sh`,
which runs A then B back-to-back on a fresh network, scrapes Prometheus into
per-run JSON, aggregates, and plots):

- **Workload**: `slow::slow(n, size)` with `n == size`, owned-object
  (`SLOW_SHARED=false`) — each transaction only does CPU work (no shared
  objects), so no congestion control or scheduling noise, so nothing but
  attestation drives the A vs B difference.
- **Computation** (`slow_size`, with `n == size`): {0, 50, 100, 200, 500} — the
  argument passed to `slow::slow(n, size)`; larger values mean more CPU work
  per transaction. But gas is bucketed (rounded up to `gas_rounding_step`), so
  the two smallest workloads fall in the same bucket and bill identically:
  `slow0` and `slow50` both sit at the floor (equal attestation cost — see
  finding 1), and the per-transaction computation cost only steps up from
  `slow100` onward.
- **Path**: `f1` = submit via fullnode (`DIRECT=false`); `v1` = pinned
  direct-to-one-validator (`DIRECT=true NUM_TARGET_VALIDATORS=1`); `v4` =
  direct to all 4 validators (`DIRECT=true NUM_TARGET_VALIDATORS=4`), the
  driver spreading submissions across them.
- **Rate** (`target_qps`): {200, 1000, 2000}.
- **Machine**: all runs on one AMD EPYC 9454P server (48 cores / 96 threads,
  251 GiB RAM, Ubuntu 24.04), running the private network in docker — 4
  validators plus 1 fullnode — with the stress client on the same host.
- 5 × 3 × 3 = **45 configurations**, **10 iterations** each; every iteration
  runs Run A (V1, attestation OFF) and Run B (V2, attestation ON).

The same matrix was then re-run on a **48-validator** network (plus 1
fullnode) on the same machine, to check how the overhead changes when
attestation spreads across a large committee instead of concentrating on 4
validators. Differences from the 4-validator campaign:

- the all-validators path becomes `v48` (`DIRECT=true
  NUM_TARGET_VALIDATORS=48`); configuration labels carry the network size as
  an `-n48` suffix (`-n4` for the 4-validator matrix);
- the host needed tuning to hold 49 node containers: larger kernel neighbor
  table limits, static container hostnames via `extra_hosts` (docker's
  embedded DNS drops lookups under the churn of that many peers), and larger
  UDP buffers.

Its results land in `results/summary_table_n48.{md,csv}` and
`results/summary_plots_n48/` (`--net 48` on the tooling below).

Unless stated otherwise, every table and figure below shows the `qps1000`
rate. All three rates were run; the effects barely depend on the rate, so one
representative rate keeps the tables and figures readable. Where a result does
depend on the rate, the prose or an extra table says so explicitly.

`run.sh` re-bootstraps a fresh genesis and network (empty DB) between A and B so
both share the same cold baseline and warmup — only attestation differs. The
monitoring stack is cycled down (without wiping its volume) and back up too,
since leaving Prometheus up across the reset would give Run B a longer warmup.
In unattended runs the TSDB is additionally wiped between A and B (Run A's
JSON is saved first); in interactive runs, it is kept, so both runs' windows
coexist in one Grafana view.

Aggregation and reporting tooling (all under `h1/` directory):

- `make_table.py` generates `results/summary_table_n<N>.md` (+
  `results/summary_table_n<N>.csv`) for one network size at a time (`--net`,
  default 4): one row per configuration, an A/B cell per
  metric (`mean ± std` over all seconds of all iterations), with the
  network-level series computed exactly as `plot.py` does (rate /
  `histogram_quantile`, per-validator collapse). Bursty queue and shedding
  gauges (in-flight count, dispatch queue, pending transactions, shed
  percentages) instead report the
  peak: max over time per iteration, averaged across iterations — their mean
  over time would hide the short spikes that actually hit a limit.
- `summary_plot.py` generates `results/summary_plots_n<N>/*.png` (same `--net`
  switch): grouped A vs B bar charts per metric, configurations on the x-axis,
  log-scale y.

> [!NOTE]
> Client-side `settlement_finality_latency` and `submit_transaction_latency` are
> recorded only on the fullnode path, so they exist for `f1` configurations
> only; the `v1`/`v4` (direct-to-validator) configurations bypass the fullnode
> and report no client-side latency.

---

### Findings (10 iterations per configuration)

Numbers below are means over all seconds of all iterations, except the bursty
queue/shedding gauges, which report peaks (see the tooling note above).
Per-configuration means are steady at light and moderate load — they vary a
few percent from run to run — and noisier on the heavy-compute configurations,
where throughput is small (up to tens of percent). Figure error bars are
±1 std (signal variability) by default; `summary_plot.py --disp sem` switches
them to the standard error of the mean.

In the figures below, blue = **A (V1, attestation off)** and red = **B (V2,
attestation on)**; the x-axis is one group per configuration
(`s<size>·<path>`, `f1` = fullnode, `v1` = pinned to one validator,
`v4`/`v48` = direct to all validators), with dashed separators between
computation sizes; the y-axis is log-scaled. To keep the figures readable,
the shedding figure shows only the heavy sizes (`slow200`/`slow500`) and the
client-side figures only the fullnode path. The tables and
`summary_table_n4.md` / `summary_table_n48.md` carry the full 45
configurations of each campaign.

---

**1. Attestation is a full execution dry-run, plus scheduling overhead that
grows with load.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `validator_attestation_latency` | Latency of `attest_transaction` (the pre-consensus dry-run) for `UserTransactionV2` transactions; spans pool wait + dry-run execution + async resume | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |
| `validator_attestation_queue_wait` | Time an attestation dry-run waits on the `spawn_blocking` pool before a worker starts it (queue wait only) | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |
| `validator_attestation_execution_latency` | Wall-clock of the attestation Move-VM dry-run itself (`spawn_blocking` closure body), excluding pool wait | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |
| `validator_attestation_async_resume_latency` | Time from the dry-run finishing on the blocking pool until the awaiting async task resumes (`spawn_blocking` join). Grows when the async runtime is saturated; full latency = queue wait + execution + this | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |
| `authority_state_internal_execution_latency` | Latency of actual certificate executions | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |
| `actual_computation_units` | Actual computation cost in gas units (CU) of attested transactions (`computation_cost` / `gas_price`), observed after execution | histogram; per-tx mean `rate(_sum)/rate(_count)` combined across validators (exact, not a quantile); averaged over all seconds of all iterations |

> [!IMPORTANT]
> The dry-run does not run on the async runtime: it is handed to a separate
> thread pool (`tokio::task::spawn_blocking`), and the async task that
> submitted it waits for the result. **Pool wait** is the time the dry-run sits
> queued before a pool thread starts it. **Async resume** is the time after the
> dry-run finishes until the waiting async task gets CPU time to continue. So
> full attestation latency = pool wait + dry-run execution + async resume.
> Under heavy load the pool's threads keep all CPU cores busy, which starves
> the async runtime — the async resume part then grows to seconds.

`validator_attestation_execution_latency` (B only — the dry-run itself, without
the pool wait and async resume around it) grows with the transaction's
computation cost and tracks the actual execution latency across the whole
sweep, matching it at the heavy end. The full attestation latency
(`validator_attestation_latency`) equals the dry-run at light compute, then
pulls away from `slow200` on — the wait and resume around the dry-run grow
with load, on every client path:

Fullnode path (`f1`):

| slow_size | attest. exec. lat. p95 | exec. lat. p95 | attest. full lat. p95 | CUs  |
| --- | --- | --- | --- | --- |
| 0   | 0.95 ms   | 2.06 ms   | 0.95 ms   | 1k    |
| 50  | 4.80 ms   | 5.87 ms   | 4.83 ms   | 1k    |
| 100 | 16.21 ms  | 20.30 ms  | 19.29 ms  | 4k    |
| 200 | 94.85 ms  | 185.32 ms | 444.29 ms | 128k  |
| 500 | 1.307 s   | 1.200 s   | 2.013 s   | 1.37M |

Direct-to-one-validator path (`v1`):

| slow_size | attest. exec. lat. p95 | exec. lat. p95 | attest. full lat. p95 | CUs  |
| --- | --- | --- | --- | --- |
| 0   | 0.95 ms   | 1.32 ms   | 0.95 ms   | 1k    |
| 50  | 4.80 ms   | 6.20 ms   | 4.85 ms   | 1k    |
| 100 | 17.37 ms  | 21.29 ms  | 20.60 ms  | 4k    |
| 200 | 78.78 ms  | 207.46 ms | 505.16 ms | 128k  |
| 500 | 990.24 ms | 999.34 ms | 2.515 s   | 1.37M |

Direct-to-all-4 path (`v4`):

| slow_size | attest. exec. lat. p95 | exec. lat. p95 | attest. full lat. p95 | CUs  |
| --- | --- | --- | --- | --- |
| 0   | 0.95 ms   | 1.21 ms   | 0.95 ms   | 1k    |
| 50  | 4.80 ms   | 6.02 ms   | 4.84 ms   | 1k    |
| 100 | 18.00 ms  | 21.19 ms  | 20.35 ms  | 4k    |
| 200 | 93.27 ms  | 221.61 ms | 503.07 ms | 128k  |
| 500 | 1.372 s   | 1.478 s   | 2.712 s   | 1.37M |

The dry-run and the real execution share the bulk of the work — load the
inputs, run the Move VM — so their latencies scale together with computation
cost. The differences are at the edges. For a no-op transaction (`slow0`) the
Move work is almost nothing and only fixed overhead remains; the real
execution (`try_execute_immediately`: lock, input load, Move VM, effects
commit) carries more of it than the attestation checks do, so it sits a little
above the dry-run (≈1.3–2.1 vs ≈0.95 ms). At mid-range compute (`slow200`) the
real execution reads about twice the dry-run — not extra work, but timing: it
runs after consensus, where a whole commit's transactions land at once and
execute in parallel, and every one of the 4 validators executes every
transaction, while each transaction is attested once, paced by client
arrivals. On this one machine that is 4× the CPU demand, so the parallel
executions share cores and each takes longer on the wall clock. At `slow500`
the machine is saturated continuously either way and the two converge. The
full attestation latency adds the scheduling around the dry-run: nothing at
light load, but from `slow200` on the pool wait and async resume grow to
dominate it (444 ms full vs 95 ms dry-run on `f1`). A heavy attested
transaction is still executed twice — once for the dry-run, once for real —
so it costs the validator roughly double.

![Attestation computation units and dry-run execution latency](h1/results/summary_plots_n4/attestation_latency_exec.png)

*Computation units, attestation dry-run execution latency (p50/p95), and actual
execution latency (p95) — findings 1–3. CUs sit at the gas floor for `slow0`
and `slow50` and step up from `slow100`; the dry-run tracks actual execution
latency across the sweep.*

The full attestation latency split into its three parts (pool wait + dry-run
execution + async resume):

Fullnode path (`f1`), p50:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.50 ms   | 0.50 ms   | 0.50 ms  | 0.50 ms   |
| 50  | 0.51 ms   | 2.96 ms   | 0.50 ms  | 2.99 ms   |
| 100 | 0.55 ms   | 6.10 ms   | 0.51 ms  | 6.52 ms   |
| 200 | 59.26 ms  | 36.24 ms  | 0.83 ms  | 113.94 ms |
| 500 | 170.36 ms | 765.24 ms | 8.69 ms  | 1.050 s   |

Fullnode path (`f1`), p99:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.99 ms   | 0.99 ms   | 0.99 ms   | 0.99 ms   |
| 50  | 2.17 ms   | 4.96 ms   | 0.99 ms   | 5.21 ms   |
| 100 | 9.44 ms   | 23.24 ms  | 2.93 ms   | 24.74 ms  |
| 200 | 597.54 ms | 135.36 ms | 67.57 ms  | 649.60 ms |
| 500 | 1.413 s   | 1.475 s   | 498.77 ms | 2.474 s   |

Direct-to-one-validator path (`v1`), p50:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.50 ms  | 0.50 ms   | 0.50 ms   | 0.50 ms   |
| 50  | 0.51 ms  | 2.97 ms   | 0.50 ms   | 3.01 ms   |
| 100 | 0.57 ms  | 6.29 ms   | 0.51 ms   | 6.82 ms   |
| 200 | 7.93 ms  | 10.86 ms  | 7.16 ms   | 60.82 ms  |
| 500 | 42.59 ms | 201.44 ms | 182.38 ms | 597.20 ms |

Direct-to-one-validator path (`v1`), p99:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.99 ms   | 0.99 ms   | 0.99 ms   | 0.99 ms   |
| 50  | 2.98 ms   | 4.97 ms   | 1.00 ms   | 6.19 ms   |
| 100 | 14.80 ms  | 23.47 ms  | 2.84 ms   | 27.92 ms  |
| 200 | 474.62 ms | 125.30 ms | 535.39 ms | 747.32 ms |
| 500 | 1.297 s   | 1.296 s   | 2.217 s   | 3.287 s   |

Direct-to-all-4 path (`v4`), p50:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.50 ms   | 0.50 ms   | 0.50 ms  | 0.50 ms   |
| 50  | 0.51 ms   | 2.98 ms   | 0.50 ms  | 3.01 ms   |
| 100 | 0.56 ms   | 6.49 ms   | 0.51 ms  | 6.92 ms   |
| 200 | 45.72 ms  | 36.77 ms  | 2.02 ms  | 117.28 ms |
| 500 | 145.01 ms | 836.25 ms | 27.83 ms | 1.276 s   |

Direct-to-all-4 path (`v4`), p99:

| slow_size | pool wait | dry-run exec | async resume | full |
| --- | --- | --- | --- | --- |
| 0   | 0.99 ms   | 0.99 ms   | 0.99 ms   | 0.99 ms   |
| 50  | 2.55 ms   | 4.96 ms   | 0.99 ms   | 5.50 ms   |
| 100 | 9.66 ms   | 23.60 ms  | 3.06 ms   | 24.42 ms  |
| 200 | 644.57 ms | 132.85 ms | 293.54 ms | 753.15 ms |
| 500 | 1.793 s   | 1.547 s   | 1.723 s   | 3.457 s   |

At light compute, every part sits at the histogram floor (≈0.5 ms) — the full
latency is just the dry-run. Under heavy compute the overhead appears, and the
two paths pay it differently. On `f1` the dry-runs queue up for a pool thread:
pool wait dominates (598 ms at `slow200` p99) while resume stays small. On `v1`
the one pinned validator attests everything; its cores saturate and finished
dry-runs wait for the starved async runtime to pick the result up — async
resume grows into the largest part at the tail (2.217 s at `slow500` p99, more
than the dry-run itself). `v4` sits between the two: submissions spread over
all 4 validators, so each validator attests only a quarter of the load, and
the resume tail lands mid-way (1.723 s at `slow500` p99 vs 0.499 s on `f1` and
2.217 s on `v1`). The parts do not sum exactly to the full column:
each column is its own percentile over different transactions, so the split is
additive at the mean, not per percentile.

![Attestation pool wait latency](h1/results/summary_plots_n4/attestation_latency_wait.png)

*Attestation pool wait (p99/p95/p50) — how long a dry-run sits queued before a
`spawn_blocking` pool thread starts it. Grows on the heavy `f1` configurations,
where dry-runs arrive faster than pool threads get CPU.*

![Attestation async resume latency](h1/results/summary_plots_n4/attestation_latency_resume.png)

*Attestation async resume (p99/p95/p50) — how long after the dry-run finishes
until the waiting async task gets CPU time to continue. The tail grows largest
on the heavy pinned (`v1`) configurations, where the one attesting validator's
cores are saturated.*

![Full attestation latency](h1/results/summary_plots_n4/attestation_latency_full.png)

*Full attestation latency (p99/p95/p50) — pool wait + dry-run execution +
async resume, the whole `attest_transaction` span.*

---

**2. Internal execution latency: unchanged by attestation.**

> [!TIP]
> Both metrics of this finding — `authority_state_internal_execution_latency`
> and `actual_computation_units` — are described in finding 1's metric table.

`authority_state_internal_execution_latency` (the real, post-consensus VM
execution) is A≈B: the p95 B/A ratio has median **1.00** across all 45
configurations (range 0.77–1.52). The deviations sit on the heavy-compute
configurations and swing in both directions — B faster on some, slower on
others — so they are load noise, not a systematic attestation cost. The
largest one (`v4` at `slow500`, 1.52) is the contention effect from finding 1:
B's dry-runs add CPU load that stretches the real execution's wall clock.
Attestation does not touch the execution path itself; its cost lives in the
pre-consensus dry-run (finding 1). Execution latency p95 (CUs are
measured on attested transactions, so they exist for B only):

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A | CUs |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 1.80 ms   | 2.06 ms   | 1.14 | 1.34 ms   | 1.32 ms   | 0.98 | 1.12 ms   | 1.21 ms   | 1.08 | 1k    |
| 50  | 5.45 ms   | 5.87 ms   | 1.08 | 5.91 ms   | 6.20 ms   | 1.05 | 5.95 ms   | 6.02 ms   | 1.01 | 1k    |
| 100 | 21.35 ms  | 20.30 ms  | 0.95 | 21.98 ms  | 21.29 ms  | 0.97 | 21.92 ms  | 21.19 ms  | 0.97 | 4k    |
| 200 | 212.18 ms | 185.32 ms | 0.87 | 222.24 ms | 207.46 ms | 0.93 | 207.40 ms | 221.61 ms | 1.07 | 128k  |
| 500 | 969.08 ms | 1.200 s   | 1.24 | 971.15 ms | 999.34 ms | 1.03 | 970.22 ms | 1.478 s   | 1.52 | 1.37M |

---

**3. Compute-unit accounting is exact.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `attested_computation_units` | Attestor's pre-consensus estimate of the computation cost in gas units (CU), for transactions that arrived as `UserTransactionV2` | histogram; per-tx mean `rate(_sum)/rate(_count)` combined across validators; averaged over all seconds of all iterations |
| `actual_to_attested_computation_units_ratio` | Ratio actual / attested computation units for attested transactions | histogram; per-tx mean `rate(_sum)/rate(_count)` combined across validators; averaged over all seconds of all iterations |

> [!TIP]
> `actual_computation_units` — the other metric of this finding — is described
> in finding 1's metric table.

Attested computation units equal actual computation units for every
owned-object configuration (ratio = 1.0), confirming attestation predicts the
computation cost precisely for these transactions. CUs are reported as the
exact per-transaction mean (`_sum`/`_count`), not a p50: the
workload is deterministic, so every transaction is identical and the mean is
the exact cost. A p50 would instead interpolate between histogram bucket edges
and land on impossible values (e.g., 850 for `slow0`, below the 1000-unit
`gas_rounding_step` floor).

---

**4. Receipt → execution latency: roughly doubles under heavy load.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `validator_transaction_execution_latency` | Validator-internal latency from receiving a transaction via `submit_tx` until it finished executing (pre-consensus check, consensus, post-consensus validation, sequencing incl. deferral, execution); excludes client/fullnode time | histogram; p50/p95/p99 (`histogram_quantile`) per validator, then max across validators (busiest); averaged over all seconds of all iterations |

`validator_transaction_execution_latency` times the whole internal pipeline on
the receiving validator — receipt via `submit_tx`, attestation, consensus,
post-consensus validation, and execution — no client/fullnode time. Median
(p50):

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 300 ms | 284 ms | 0.95 | 244 ms  | 225 ms  | 0.92 | 250 ms  | 249 ms  | 1.00 |
| 50  | 299 ms | 294 ms | 0.98 | 245 ms  | 288 ms  | 1.18 | 264 ms  | 275 ms  | 1.04 |
| 100 | 290 ms | 305 ms | 1.05 | 265 ms  | 286 ms  | 1.08 | 297 ms  | 295 ms  | 0.99 |
| 200 | 787 ms | 1.37 s | 1.75 | 1.60 s  | 2.93 s  | 1.82 | 1.60 s  | 2.43 s  | 1.52 |
| 500 | 4.23 s | 8.39 s | 1.99 | 10.83 s | 17.95 s | 1.66 | 11.25 s | 13.29 s | 1.18 |

At light load the pipeline is ≈230–300 ms and A≈B — dominated by consensus,
with attestation (a few ms at these sizes) lost in the noise. At heavy compute
B runs ≈1.7–2.0× A (`slow500-f1` 4.23 s → 8.39 s), because attestation adds a
second full execution before consensus (finding 1) and, under load, the extra
work compounds through queueing. p95 tracks the same (`slow500-f1` 6.8 s →
13.1 s). Two path effects stand out. The direct paths (`v1`, `v4`) start from
a far higher A baseline under heavy compute (≈11 s vs 4.2 s on `f1` at
`slow500`) — without the fullnode in between, the client pushes into consensus
at full rate and the backlog builds up on the receiving side. And B's relative
cost shrinks as attestation spreads: B/A at `slow500` is 1.99 on `f1`, 1.66 on
`v1`, 1.18 on `v4`, where each validator attests only a quarter of the load.

![Receipt → execution latency](h1/results/summary_plots_n4/receipt_to_exec_latency.png)

*Validator-internal receipt → executed latency — the pure validator-internal
pipeline, with no client/fullnode time.*

---

**5. Checkpoint creation lag: attestation moves the backlog ahead of
consensus.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `checkpoint_creation_latency` | Latency from consensus commit timestamp to local checkpoint creation in milliseconds | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |

`checkpoint_creation_latency` times consensus commit created → checkpoint
built (values are seconds, despite the help text saying milliseconds). The
builder can only seal a checkpoint once that commit's transactions have
executed, so the lag is a direct view of the post-consensus execution backlog.
p95:

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 562 ms | 655 ms  | 1.17 | 499 ms  | 251 ms  | 0.50 | 243 ms  | 239 ms  | 0.98 |
| 50  | 401 ms | 520 ms  | 1.30 | 268 ms  | 584 ms  | 2.18 | 511 ms  | 723 ms  | 1.42 |
| 100 | 274 ms | 322 ms  | 1.18 | 293 ms  | 258 ms  | 0.88 | 695 ms  | 273 ms  | 0.39 |
| 200 | 3.12 s | 4.47 s  | 1.43 | 15.57 s | 5.59 s  | 0.36 | 15.29 s | 11.74 s | 0.77 |
| 500 | 9.99 s | 11.56 s | 1.16 | 30.21 s | 11.74 s | 0.39 | 32.39 s | 26.36 s | 0.81 |

At light compute the lag is a steady ≈0.2–0.7 s on all paths. Under heavy
compute the paths diverge, in two separate ways.

First, the A side splits by submission route, not by spreading: both direct
paths pile up a huge post-consensus backlog (A at `slow500`: 30.21 s on `v1`,
32.39 s on `v4`) while the fullnode path stays at 9.99 s. The fullnode acts as
an admission buffer — its own transaction driver queues and paces what enters
consensus — and spreading the direct submissions over all 4 validators (`v4`)
does not substitute for it.

Second, on the B side attestation moves the backlog ahead of consensus, and
the strength of that shift follows how concentrated the attestation is. On
`v1` one validator attests everything and intake is throttled hardest: A lags
far more than B (30.21 vs 11.74 s; at p50 16.73 vs 2.31 s). On `v4` each
validator attests a quarter and the shift is half-hearted (B/A 0.77–0.81). On
`f1` attestation is spread the same way but B also keeps the deeper execution
backlog, so B lags slightly more than A (1.2–1.4×). Without attestation the
load goes straight into consensus and the backlog piles up after it — exactly
where checkpoints wait; with attestation, each transaction first spends time
in the dry-run while the client holds a bounded number in flight (finding 4's
receipt→execution shows that side: B ≈1.7× A on `v1`, ≈1.2× on `v4`).
Attestation does not shrink the total backlog — it moves it from after
consensus, where checkpoints wait on it, to before consensus, and the more
concentrated the attestation, the stronger the move.

![Checkpoint creation lag](h1/results/summary_plots_n4/checkpoint_creation_latency.png)

*Checkpoint creation lag (p99/p95/p50) — consensus commit created → checkpoint
built. Note the heavy direct-path (`v1`, `v4`) configurations: A (attestation
off) lags far more than B, because its backlog sits after consensus.*

---

**6. Post-consensus validation latency: unaffected by attestation.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `post_consensus_validation_latency` | Latency of `validate_and_resolve_conflicts` over one consensus commit's user transactions (Checks #0-#3 plus owned-object conflict resolution) | histogram; p50/p95/p99 (`histogram_quantile`) over buckets combined across validators; averaged over all seconds of all iterations |

`validate_and_resolve_conflicts` (the post-consensus pass) is where attestation
adds Check #3 — attestor verification plus cost bounds. But that's a few integer
comparisons per tx; the pass is dominated by the already-executed cache lookup
(Check #1) and owned-object lock/conflict resolution. All paths:

Fullnode path (`f1`):

| slow_size | p50 A | p50 B | p50 B/A | p95 A | p95 B | p95 B/A |
| --- | --- | --- | --- | --- | --- | --- |
| 0   | 2.9 ms | 2.8 ms | 0.96 | 5.7 ms | 5.1 ms | 0.89 |
| 50  | 2.9 ms | 2.8 ms | 0.96 | 5.2 ms | 4.9 ms | 0.95 |
| 100 | 2.5 ms | 2.0 ms | 0.80 | 4.9 ms | 4.8 ms | 0.98 |
| 200 | 2.3 ms | 1.1 ms | 0.48 | 19 ms  | 12 ms  | 0.63 |
| 500 | 2.1 ms | 1.7 ms | 0.82 | 26 ms  | 24 ms  | 0.94 |

Direct-to-one-validator path (`v1`):

| slow_size | p50 A | p50 B | p50 B/A | p95 A | p95 B | p95 B/A |
| --- | --- | --- | --- | --- | --- | --- |
| 0   | 2.9 ms | 2.9 ms | 1.00 | 5.9 ms | 5.1 ms | 0.85 |
| 50  | 2.9 ms | 2.8 ms | 0.98 | 5.4 ms | 5.0 ms | 0.93 |
| 100 | 2.5 ms | 2.2 ms | 0.89 | 5.4 ms | 4.8 ms | 0.90 |
| 200 | 3.3 ms | 0.6 ms | 0.20 | 22 ms  | 21 ms  | 0.95 |
| 500 | 7.0 ms | 0.4 ms | 0.06 | 69 ms  | 15 ms  | 0.21 |

Direct-to-all-4 path (`v4`):

| slow_size | p50 A | p50 B | p50 B/A | p95 A | p95 B | p95 B/A |
| --- | --- | --- | --- | --- | --- | --- |
| 0   | 3.0 ms | 2.9 ms | 0.98 | 5.3 ms | 4.9 ms | 0.93 |
| 50  | 2.9 ms | 2.8 ms | 0.95 | 5.3 ms | 4.9 ms | 0.92 |
| 100 | 2.5 ms | 2.2 ms | 0.87 | 4.9 ms | 4.8 ms | 0.98 |
| 200 | 3.6 ms | 3.3 ms | 0.90 | 19 ms  | 22 ms  | 1.13 |
| 500 | 13 ms  | 7.2 ms | 0.56 | 61 ms  | 57 ms  | 0.95 |

p50 is ≈2–3 ms at light load, and the B/A column has no consistent direction —
it swings from 0.06 to 1.13, worst on the direct-path heavy configs. That's
noise, not
an attestation effect: the pass is timed per consensus commit, so heavy configs
(low throughput) get few samples. p95 rises under load (≈5 ms → 12–69 ms) on
both A and B, from contention on the pass. Attestation's Check #3 is lost in the
noise; its cost is pre-consensus (finding 1), not here.

![Post-consensus validation latency](h1/results/summary_plots_n4/post_consensus_validation_latency.png)

*Time in `validate_and_resolve_conflicts`; Check #3 (attestor verification) is
the attestation-added work on this path.*

---

**7. Submit latency (fullnode path): a fixed per-transaction addition.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `transaction_driver_submit_transaction_latency` | Time in seconds to successfully submit a transaction to a validator | histogram; p50/p95/p99 (`histogram_quantile`), fullnode client series only; averaged over all seconds of all iterations |

> [!NOTE]
> The full help text continues: "Includes all retries and measures from the
> start of submission until a validator accepts the transaction." The timer
> runs on the fullnode's `TransactionDriver`, and the validator's `submit_tx`
> RPC responds only after the transaction passed the overload check, the whole
> attestation span finished (pool wait + dry-run + async resume — the
> attestation payload is needed to build the consensus transaction), and the
> transaction was handed to the consensus adapter. It does not wait for
> consensus sequencing — that time is in settlement finality (finding 8), not
> here.

B's submit `p50` exceeds A's by roughly the full attestation latency
(`validator_attestation_latency`, pool wait + dry-run execution + async
resume — the submit RPC returns only after the whole attestation span), so the
*ratio* is largest where the baseline is smallest (low rate / low computation
cost): `slow0-f1-q200` 4.7 ms → 14.5 ms (3.1×), `slow500-f1-q200` 25.2 ms →
674 ms
(27×, i.e. +649 ms ≈ the full attestation p50, 616 ms at that configuration).
At high rate the queueing baseline dominates and the ratio shrinks (≈1.1–5×).
The *added* latency (B − A) equals the full attestation span only at low rate;
under load the dry-runs queue and it grows well past that (`slow500-f1-q2000`
submit reaches 3.8 s).

Submit p50 (ms) on the fullnode path (A = attestation off, B = on):

| slow_size | q200 A | q200 B | q200 B/A | q1000 A | q1000 B | q1000 B/A | q2000 A | q2000 B | q2000 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 4.7  | 14.5 | 3.1  | 3.7  | 4.2  | 1.1 | 3.6  | 3.8  | 1.1 |
| 50  | 4.9  | 24.9 | 5.1  | 3.7  | 10.2 | 2.8 | 3.1  | 6.6  | 2.1 |
| 100 | 4.4  | 26.3 | 6.0  | 3.3  | 15.0 | 4.5 | 5.0  | 21.6 | 4.3 |
| 200 | 3.7  | 41.4 | 11.1 | 83.0 | 261  | 3.1 | 106  | 379  | 3.6 |
| 500 | 25.2 | 674  | 26.8 | 386  | 2044 | 5.3 | 1007 | 3760 | 3.7 |

Full attestation latency p50 (ms) at the same configurations, to check the
addition directly (submit A + full attestation ≈ submit B):

| slow_size | q200 | q1000 | q2000 |
| --- | --- | --- | --- |
| 0   | 0.5  | 0.5  | 0.5  |
| 50  | 3.0  | 3.0  | 2.8  |
| 100 | 8.6  | 6.5  | 8.7  |
| 200 | 33.1 | 114  | 116  |
| 500 | 616  | 1050 | 1381 |

The addition holds at low rate — e.g. `slow500-f1-q200`: 25.2 + 616 ≈ 674, and
`slow200-f1-q200`: 3.7 + 33.1 ≈ 41.4. At high rate B's submit grows past the sum
(`slow500-f1-q2000`: 1007 + 1381 = 2389 vs 3760 measured) — the extra is
queueing
on the loaded validator beyond the attestation span itself.

![Submit-transaction latency](h1/results/summary_plots_n4/submit_latency.png)

*Client submit latency, fullnode path only — finding 7.*

---

**8. Settlement finality latency: the client sees the same doubling.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `transaction_driver_settlement_finality_latency` | Settlement finality latency observed from transaction driver | histogram; p50/p95/p99 (`histogram_quantile`), fullnode client series only; averaged over all seconds of all iterations |

`settlement_finality_latency` is the client's submit→finality time (fullnode
path only). It's the end-to-end view of the internal pipeline (finding 4) plus
network and finality, so it moves the same way. Fullnode path:

| slow_size | p50 A | p50 B | p50 B/A | p95 A | p95 B | p95 B/A |
| --- | --- | --- | --- | --- | --- | --- |
| 0   | 253 ms | 252 ms | 1.00 | 367 ms | 374 ms  | 1.02 |
| 50  | 259 ms | 258 ms | 1.00 | 359 ms | 351 ms  | 0.98 |
| 100 | 264 ms | 270 ms | 1.02 | 373 ms | 390 ms  | 1.05 |
| 200 | 804 ms | 1.25 s | 1.56 | 1.20 s | 2.00 s  | 1.67 |
| 500 | 4.26 s | 7.53 s | 1.77 | 7.08 s | 11.65 s | 1.65 |

At light load B≈A (≈250 ms, dominated by consensus/finality; attestation is
negligible). At heavy compute B runs ≈1.6–1.8× A (`slow500` 4.26 s → 7.53 s
p50), the doubling from finding 4 carried through to what the client observes.

![Settlement finality latency](h1/results/summary_plots_n4/settlement_finality_latency.png)

*Client settlement-finality latency, fullnode path only.*

---

**9. CPU: attestation adds ≈30 % busy cores.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `container_cpu_usage_seconds_total` | cadvisor (no in-repo help): cumulative CPU seconds consumed by the container | counter; `rate()` → busy cores, max across validators (busiest); averaged over all seconds of all iterations |
| `container_memory_rss` | cadvisor (no in-repo help): container resident set size (RSS) in bytes | gauge; max across validators (busiest); averaged over all seconds of all iterations |
| `node_cpu_seconds_total` | node-exporter (no in-repo help): seconds each CPU spent in each mode | counter; `rate()` over non-idle modes summed to whole-machine busy cores; averaged over all seconds of all iterations |

Per-validator CPU (busiest validator, cadvisor) B/A median = **1.28×** (range
0.99–2.23×) — e.g. `slow100-f1` 8.7 → 11.1 cores, `slow500-f1`
20.9 → 24.7 cores. Consistent with the extra dry-run execution.

Busiest-validator CPU (cores) by slow_size:

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 2.7  | 2.8  | 1.05 | 3.0  | 3.3  | 1.09 | 2.7  | 2.9  | 1.05 |
| 50  | 5.3  | 6.4  | 1.21 | 5.7  | 8.1  | 1.43 | 5.3  | 6.7  | 1.26 |
| 100 | 8.7  | 11.1 | 1.28 | 9.1  | 14.6 | 1.60 | 9.0  | 11.8 | 1.30 |
| 200 | 18.7 | 21.0 | 1.12 | 21.1 | 31.9 | 1.51 | 20.2 | 24.4 | 1.21 |
| 500 | 20.9 | 24.7 | 1.19 | 23.0 | 35.9 | 1.56 | 23.7 | 24.7 | 1.04 |

The pinned path (`v1`) rises more (up to ≈1.6×) than the fullnode path
(≈1.1–1.3×), because that one validator attests every transaction, while on
`f1` the attestation work is spread across the four. `v4` confirms it is the
spreading that matters, not the fullnode: submitting directly to all 4 keeps
the busiest validator at fullnode-path levels (B ≈ 24.7 cores at `slow500`,
matching `f1` and well below `v1`'s 35.9).

Busiest-validator memory RSS (GB) by slow_size:

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 0.8 | 0.8 | 1.00 | 0.8 | 0.8 | 0.99 | 0.8 | 0.8 | 1.00 |
| 50  | 0.8 | 0.7 | 0.99 | 0.8 | 0.8 | 0.99 | 0.8 | 0.8 | 1.01 |
| 100 | 0.7 | 0.7 | 1.00 | 0.8 | 0.8 | 1.01 | 0.8 | 0.8 | 0.99 |
| 200 | 0.7 | 0.8 | 1.08 | 0.8 | 0.9 | 1.07 | 0.8 | 0.8 | 1.05 |
| 500 | 0.5 | 0.6 | 1.29 | 0.5 | 0.7 | 1.38 | 0.5 | 0.7 | 1.25 |

Memory stays small and roughly flat (≈0.7–0.8 GB); attestation barely moves it —
the heavy-config bumps are on ≈0.5–0.9 GB and noisy. Attestation's cost is CPU,
not memory.

![CPU and memory](h1/results/summary_plots_n4/resources.png)

*Whole-machine host CPU and busiest-validator CPU / memory (RSS) — finding 9.*

---

**10. Throughput: no penalty at normal load; a fullnode cost at heavy compute.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `transactions_included_in_checkpoint` | Transactions included in a checkpoint | counter; `rate()` → finalized TPS, mean across validators (replicated); averaged over all seconds of all iterations |
| `validator_attestations_total` | Number of attestations performed (dry-runs that completed without panicking) | counter; `rate()` → attestations/s, max across validators (busiest); averaged over all seconds of all iterations |

Finalized TPS (`transactions_included_in_checkpoint`) is statistically
identical A vs B at normal load — median `(B−A)/A = −0.4 %` across all 45
configurations, within the few-percent run-to-run noise.

Finalized TPS by slow_size (A = attestation off, B = on; `slow500`
is small and noisy):

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 994  | 987  | 0.99 | 1024 | 1024 | 1.00 | 1022 | 1022 | 1.00 |
| 50  | 1010 | 1003 | 0.99 | 1022 | 1020 | 1.00 | 1022 | 1022 | 1.00 |
| 100 | 1023 | 1019 | 1.00 | 1010 | 1024 | 1.01 | 1022 | 1022 | 1.00 |
| 200 | 747  | 584  | 0.78 | 602  | 636  | 1.06 | 589  | 564  | 0.96 |
| 500 | 129  | 104  | 0.81 | 105  | 94   | 0.90 | 88   | 79   | 0.89 |

Caveat: the −0.4 % median is the normal-load result. On the fullnode path the
cost grows with compute — B/A ≈ 0.78 at `slow200`, ≈ 0.81 at `slow500` — while
the direct paths pay little or nothing (`v1` 1.06/0.90 and `v4` 0.96/0.89 at
`slow200`/`slow500`),
even though it sends every attestation to a single validator. Why the fullnode
path pays more is not established here (both sit at ≈76–85/96 host CPU, so it
is not spare capacity); it needs a dedicated look.

attestations / sec (the busiest validator's rate) shows how the two client
paths spread attestation work. On the pinned path (`v1`) one validator attests
nearly all traffic, so its rate tracks the full transaction rate; on the
fullnode path (`f1`), the fullnode spreads submissions across the four
validators, so the busiest one attests only its share — about half the pinned
rate at light load (`slow0`: 484 vs 994 /s) and roughly a fifth under
heavy compute (`slow200`: 306 vs 1546 /s). `v4` spreads just as evenly
without a fullnode in the picture (busiest ≈ 500 /s at light load, 426 at
`slow200`) — the driver's validator selection balances the load on its own.
Finalized TPS is approximately the same on all paths, so this is about how
attestation work is spread, not throughput.

attestations / sec by path (busiest validator):

| config          | `f1` | `v1` | `v4` | v1/f1 |
| ---             | ---  | ---  | ---  | ---   |
| `slow0`   | 484  | 994  | 500  | 2.1×  |
| `slow100` | 501  | 993  | 503  | 2.0×  |
| `slow200` | 306  | 1546 | 426  | 5.0×  |
| `slow500` | 74   | 516  | 96   | 7.0×  |

---

**11. No post-consensus validation drops.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `consensus_handler_validation_dropped_transactions` | Number of `UserTransactionV1`/`UserTransactionV2` transactions dropped by post-consensus validation | counter; `rate()` → drops/s, mean across validators; averaged over all seconds of all iterations |

`consensus_handler_validation_dropped_transactions` is ≈0 on both the attested
(V2) and unattested (V1) paths, across every configuration.

![Throughput, attestation rate, and validation-drop rate](h1/results/summary_plots_n4/TPS.png)

*Finalized TPS, attestations / sec, and post-consensus validation-drops / sec —
findings 10 and 11. TPS is A≈B; no validation drops on either path.*

---

**12. Execution queues and backpressure: deeper backlog under heavy load.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `execution_queueing_delay_s` | Queueing delay between a transaction is ready for execution until it starts executing | histogram; p50/p95/p99 (`histogram_quantile`) per validator, then max across validators; averaged over all seconds of all iterations |
| `execution_driver_dispatch_queue` | Number of transaction pending in execution driver dispatch queue | gauge; max across validators (busiest); peak — max over time per iteration, averaged across iterations |
| `transaction_manager_num_pending_certificates` | Number of certificates pending in `TransactionManager`, with at least 1 missing input object | gauge; max across validators (busiest); peak — max over time per iteration, averaged across iterations |

Under load, execution work queues up. Headline signal: queue-delay p95 (how long
a tx waits before executing); dispatch-queue depth and pending-tx count track
it:

| slow_size | f1: A | f1: B | f1 B/A | v1: A | v1: B | v1 B/A | v4: A | v4: B | v4 B/A |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0   | 5 ms   | 5 ms   | 1.00 | 5 ms   | 5 ms   | 0.98 | 5 ms   | 5 ms   | 1.00 |
| 50  | 10 ms  | 10 ms  | 1.08 | 11 ms  | 11 ms  | 0.98 | 10 ms  | 11 ms  | 1.09 |
| 100 | 27 ms  | 29 ms  | 1.05 | 28 ms  | 26 ms  | 0.92 | 29 ms  | 27 ms  | 0.94 |
| 200 | 508 ms | 997 ms | 1.96 | 1.91 s | 1.81 s | 0.95 | 1.83 s | 1.70 s | 0.93 |
| 500 | 2.44 s | 3.41 s | 1.39 | 5.21 s | 5.33 s | 1.02 | 5.23 s | 7.35 s | 1.41 |

Light configs barely queue (≈5–29 ms, A≈B). On the fullnode path B carries a
deeper backlog under heavy compute — queue-delay 1.4–2.0× A, and the
dispatch-queue peak grows the same way (`slow200-f1` 877 → 1280) — because
attestation's extra execution piles onto a busy pipeline. The direct paths
show no clean effect on queue delay (`v1` B/A 0.92–1.02; `v4` mixed,
0.93–1.41), but their A sides carry large pending-transactions outliers
(`slow200` peaks: 1482 pending in A vs 74 in B on `v1`, 2308 vs 131 on `v4`) —
the same picture as finding 5: without attestation the direct paths' backlog
sits after consensus.

![Execution queues and backpressure](h1/results/summary_plots_n4/queues.png)

*Execution dispatch queue, pending transactions, and execution queue delay
(p95).*

---

**13. Post-consensus load shedding: sheds under heavy compute on both
paths.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `consensus_handler_load_shedding_dropped_transactions` | Number of user transactions dropped by post-consensus load shedding, based on the quorum load shedding percentage | counter; `rate()` → drops/s, max across validators (busiest); averaged over all seconds of all iterations |
| `consensus_handler_load_shedding_percentage` | Stake-weighted quorum (2f+1) load shedding percentage enforced on user transactions in the most recent consensus commit. 0 when the P-COOL flow is disabled | gauge; max across validators; peak — max over time per iteration, averaged across iterations |
| `authority_load_shedding_percentage` | This authority's locally computed load shedding percentage. In the P-COOL flow this is the value broadcast to peers, not necessarily the rate enforced (see `consensus_handler_load_shedding_percentage`) | gauge; max across validators; peak — max over time per iteration, averaged across iterations |

Light and moderate configurations (`slow0`–`slow100`) barely shed — only
small bursts at `qps2000` (percentages of a few percent, drops up to ≈7/s on
`slow0-f1-q2000` A). The heavy configurations:

| config | A drops/s | A quorum % | A local % | B drops/s | B quorum % | B local % |
| --- | --- | --- | --- | --- | --- | --- |
| `slow200-f1` | 0    | 0    | 2.2  | 0    | 0    | 1.1  |
| `slow200-v1` | 10.2 | 6.5  | 22.7 | 0    | 0    | 20.1 |
| `slow200-v4` | 22.2 | 12.2 | 27.9 | 6.9  | 6.3  | 17.7 |
| `slow500-f1` | 1.1  | 4.1  | 10.4 | 6.8  | 26.4 | 41.8 |
| `slow500-v1` | 13.9 | 30.7 | 45.6 | 2.8  | 17.9 | 60.7 |
| `slow500-v4` | 12.1 | 28.4 | 38.6 | 4.8  | 42.4 | 70.2 |
| `slow200-f1-q2000` | 1.8  | 1.9  | 11.6 | 27.1 | 15.4 | 27.6 |
| `slow200-v1-q2000` | 71.1 | 27.1 | 31.1 | 3.0  | 2.5  | 24.7 |
| `slow200-v4-q2000` | 71.3 | 26.2 | 31.5 | 52.3 | 27.4 | 37.1 |
| `slow500-f1-q2000` | 3.6  | 17.4 | 25.2 | 2.1  | 18.5 | 47.3 |
| `slow500-v1-q2000` | 19.8 | 62.9 | 72.8 | 0.9  | 20.1 | 51.6 |
| `slow500-v4-q2000` | 25.1 | 71.1 | 78.6 | 0.7  | 8.8  | 63.8 |

Under heavy compute all paths shed: the percentages rise on A and B alike
(the locally broadcast value runs ahead of the enforced quorum value, as
expected — the quorum needs 2f+1 validators to agree), and all drop
transactions. The paths differ in degree, not kind. On the direct paths A
drops far more than B (71.1 vs 3.0 /s at `slow200-v1-q2000`, 25.1 vs 0.7 /s at
`slow500-v4-q2000`): attestation throttles admission, so less backlog reaches
the post-consensus dropper (finding 5). How much less follows the attestation
concentration: `v1` throttles hardest and its B drops least (0.9–3.0 /s); on
`v4`, where each validator attests only a quarter, the throttling is weaker
and B can still drop heavily (52.3 /s at `slow200-v4-q2000`). On the fullnode
path the order can flip outright (1.8 vs 27.1 /s at `slow200-f1-q2000`) —
there B carries the deeper execution backlog (finding 12), and its shed
percentages run higher.

![Post-consensus load shedding](h1/results/summary_plots_n4/load_shedding_post_consensus.png)

*Post-consensus load shedding: drops / sec, enforced quorum shed %, and locally
broadcast shed % (peaks). A dominates the drops on the pinned path; B can
dominate on the fullnode path. The largest drops land at `qps2000` (see the
table above).*

---

**14. Pre-consensus load shedding: quiet until the heaviest pinned
configuration hits the submit semaphore.**

| metric | codebase description | aggregation |
| --- | --- | --- |
| `transaction_overload_sources` | Number of times each source indicates transaction overload | counter with a `source` label (`consensus_graduated` / `consensus_max_pending` / `consensus_semaphore`); `rate()` → rejections/s per source, max across validators; averaged over all seconds of all iterations |
| `validator_service_num_rejected_tx_during_overload` | Number of rejected transaction due to system overload | counter; `rate()` → rejections/s summed over error types, max across validators; averaged over all seconds of all iterations |
| `consensus_queue_load_shedding_percentage` | Percentage of transactions shed due to consensus queue length. Separate admission-control signal, not an input to `authority_load_shedding_percentage` | gauge; max across validators; peak — max over time per iteration, averaged across iterations |
| `sequencing_certificate_inflight` | The inflight requests to sequence certificates | gauge, one series per transaction type; summed per validator = `num_inflight` (the value the graduated / max-pending limits gate on), max across validators; peak — max over time per iteration, averaged across iterations |

`check_system_overload` rejects a transaction before consensus when the
consensus queue is saturated, labeled by which limit tripped: the graduated
soft band, the `max_pending_transactions` hard limit (20000), or the submit
semaphore (`max_pending_transactions × 2 / committee size` = 10000 for this
4-validator network). Across the whole matrix it fired in exactly one
configuration — B at `slow500-v1-qps2000`; no other configuration, at any
rate, rejected a single transaction pre-consensus:

| config | A `num_inflight` | B `num_inflight` | B graduated/s | B semaphore/s | B cons-queue % |
| --- | --- | --- | --- | --- | --- |
| `slow500-v1-q2000` | 3667 | 9726 | 32.1 | 126.8 | 4.7 |

`num_inflight` (transactions submitted to consensus but not yet sequenced) peaks
higher in B than A on every heavy configuration — under attestation each
transaction stays in the submit pipeline longer, so more sit in flight at
once. At `slow500-v1-qps2000` B's peak reaches ≈9700 ≈ the 10000-permit submit
semaphore, and rejections fire: mostly `consensus_semaphore` (≈127/s) with
some `consensus_graduated` (≈32/s), and `consensus_max_pending` never — the
semaphore is reached first and holds `num_inflight` below the 20000 hard limit.
The totals cross-check: rejections/s (≈154) ≈ graduated + semaphore. A never
sheds pre-consensus at any configuration, and neither does `v4` at any load:
spreading the submissions keeps every validator's `num_inflight` at ≈2100 or
less, far from the semaphore — the pre-consensus limits only come into play
when the load pins to a single validator.

---

### H4 — safety (pass/fail)

**PASS.** All safety counters are zero across all runs (checkpoint forks,
inconsistent state hash, double-spend, attestation task panics, soft-lock
equivocation), and no validator crashed, restarted, or OOM'd.

| metric | codebase description | aggregation |
| --- | --- | --- |
| `split_brain_checkpoint_forks` | Number of checkpoints that have resulted in a split brain | counter; max across validators over the whole window (H4 requires 0) |
| `remote_checkpoint_forks` | Number of remote checkpoints that forked from local checkpoints | counter; max across validators over the whole window (H4 requires 0) |
| `global_state_hash_inconsistent_state` | 1 if accumulated live object set differs from `GlobalStateHasher` root state hash for the previous epoch | gauge; max across validators over the whole window (H4 requires 0) |
| `total_client_double_spend_attempts_detected` | Total number of client double spend attempts that are detected | counter; max over the whole window (H4 requires 0) |
| `validator_attestation_task_panics` | Number of attestation dry-runs that panicked (surfaced as a `JoinError`) | counter; max across validators over the whole window (H4 requires 0) |
| `validator_service_num_rejected_tx_soft_lock_conflict` | Number of transactions rejected due to pre-consensus soft lock conflict on owned objects | counter; max across validators over the whole window (H4 requires 0) |

> [!NOTE]
> These results use two temporary post-consensus-validation fixes (one per
> transaction path): each keeps a sequenced transaction when an owned input is
> not yet available (`ObjectNotFound`) rather than dropping it, since a drop
> there is per-node and would fork the checkpoint. Both fixes come from branch
> `protocol-research/fix/attestation-coin-deny-post-consensus-drop-fork` and
> are cherry-picked into the branch tested here. The
> proper fix routes such deterministic failures to a cancelled-execution status
> — tracked in
> [iota-private#438](https://github.com/iotaledger/iota-private/issues/438).

---

### Takeaway

Attestation's cost is a **pre-consensus execution dry-run plus the scheduling
around it**. The dry-run itself tracks the real execution latency — a heavy
attested transaction is executed twice, roughly doubling the validator's work
(≈+30 % busy cores). At light load that is the whole story (sub-millisecond
overhead); under load the pool wait and async resume around the dry-run grow
to dominate the attestation time (findings 1, 9). Compute-unit accounting is
exact, and actual execution, post-consensus validation, and throughput at
normal load are untouched (findings 2, 3, 6, 10).

The costs surface under heavy compute. The client pays the full attestation
span on every fullnode submit, and end-to-end latency — receipt→execution and
settlement finality — roughly doubles; on the fullnode path throughput also
dips (B/A ≈ 0.78–0.81) and the execution backlog deepens (findings 4, 7, 8,
12). But attestation also *relocates* load: by slowing admission it moves the
backlog from after consensus to before it — checkpoint lag on the direct paths
is far smaller with attestation on, while `num_inflight` grows until the
heaviest pinned configuration reaches the submit semaphore and pre-consensus
shedding fires (findings 5, 14). The strength of that shift follows how
concentrated the attestation is (pinned strongest, spread-direct weaker,
fullnode weakest), and the fullnode itself acts as an admission buffer: with
attestation off, the direct paths — spread or pinned — build several times
its post-consensus backlog (finding 5). One observation deserves follow-up: the
fullnode-path throughput dip is unexplained (finding 10). With the temporary
post-consensus fixes in place, there are no validation drops or checkpoint
forks on either path (finding 11, H4 PASS).

Full per-configuration numbers: `h1/results/summary_table_n4.md`. Figures:
`h1/results/summary_plots_n4/*.png`.
