# H2 — congestion control mode (W5: slow shared-object; count vs units)

**Goal:** measure and report the difference in throughput and latency between
the two per-object congestion control modes.

> [!TIP]
> `TotalTxCount` is the mode every network runs today: it counts each
> transaction as 1 against a per-object, per-commit base limit of 10,
> regardless of the transaction cost. `TotalComputationUnits` counts each
> transaction as its attested **computation units** (CUs) against a base limit
> given in the same units.
>
> Each *configuration* — one workload at one limit and rate — is run for ten
> iterations, and every iteration runs the same load twice: Run A in
> `TotalTxCount`, Run B in `TotalComputationUnits`.

## Contents

- [TL;DR](#tldr)
- [Experiments as run](#experiments-as-run)
- [Findings](#findings)
  - [Fixed-cost experiments](#fixed-cost-experiments)
  - [Mixed-cost experiments](#mixed-cost-experiments)
- [H4 — safety (pass/fail)](#h4--safety-passfail)
- [Summary](#summary)
- [Potential next steps](#potential-next-steps)

---

## TL;DR

**While every transaction costs the same, the two modes behave identically. They
differ once a commit carries transactions of different cost, and there a well
chosen unit limit does better than the count limit — on throughput,
cancellations, and checkpoint lag at the same time (finding 6).**

- Transactions on the object execute one after another, and a new commit
  arrives every ≈50 ms, so the best limit is the one that puts about 50 ms of
  work into each commit: admit less and execution on the object finishes early
  and sits idle, admit more and the leftover piles up.
- At 1K/100K (cheap transactions of 1,000 CUs mixed with expensive ones of
  100,000 CUs) executing one expensive transaction takes ≈34 ms, so a good unit
  limit fits a single expensive transaction per commit and leaves the rest for
  cheap ones: that puts the limit between 100,000 and 200,000 CUs.
- A unit limit of 100K–150K CUs gives 3–4× the count limit's throughput, about
  half its cancellations or fewer, and checkpoint lag no higher than the count
  limit's, flat over a 300 s run where the count limit's checkpoint lag climbs
  from under 1 s to 6–7 s.
- The count limit's equivalent, `10 × mean cost`, is not the best limit. It is
  set to admit the same ten transactions the count limit does, so it inherits
  the same flaw: when ten transactions of the mix take more than 50 ms to run,
  both modes admit too much.

**The rule — a good unit limit fits one expensive transaction per commit and
gives the rest to cheap ones — was checked three ways, and none of them changed
it:** production's overshoot on, the submission rate doubled, and five-minute
runs (finding 6).

- Doubling the rate also shows that where the client was the limit, the unit
  limit gains more when more is offered: at 1K/100K with a 150K limit, its
  success tps rises from 4.1× to 5.1× the count limit's.
- The five-minute runs show that a well-chosen unit limit keeps checkpoint lag
  flat where the count limit does not: at 1K/100K with a 150K limit, Run B's
  checkpoint lag stays at 0.3–0.6 s in every minute of a 300 s run, while Run
  A's, under the count limit, climbs from 0.7 s to 7.3 s.

**The rule holds on a second machine, but the best limit changes.** The three
limit ladders were rerun on a personal workstation that executes the same
transactions 4.6× faster. What a limit has room for does not depend on the
machine — it is a sum of transactions' CUs. Which limit is best does depend on
the machine: it is the one whose transactions take about 50 ms to execute, and
how many transactions fit in 50 ms changes with how fast the machine runs them.
A limit between one and two expensive transactions' cost is the safe choice on
both machines; on the faster one, larger limits do as well on success tps and
checkpoint lag and let far more expensive transactions through.

**Neither limit stops the object from being overloaded because neither
measures time.** From 2,000 CUs per transaction upward, a count limit of 10
admits more than can execute on one shared object: at 2,000 CUs it admits
200 tx/s against a drain rate of 179, and from 5,000 CUs the gap is wide — the
drain rate is 118 tx/s there and 3.8 tx/s at 5,000,000. Success tps sits
at the drain rate either way, so no setting of either mode raises it — the
limit only picks which failure the users get, cancellations or a queue
(findings 3, 4, 5). *However, a limit in CUs can at least be set per cost, which
a count of 10 cannot.*

**When every transaction costs the same, the two modes give the same numbers.**
Across the 12 configurations where Run B's limit is exactly ten times the
transaction's CUs, success tps matches at B/A = 1.005 (0.984 to 1.025),
and cancellations, checkpoint lag, and settlement latency match the same way
(findings 1, 2). A network switching modes at ten times the average CUs loses
nothing, and gains nothing, while its traffic stays uniform.

**All 2,620 runs passed the safety check (H4):** no checkpoint fork, no
inconsistent state, no double spend, no attestation panic, no soft-lock
equivocation, and no validator crash, restart, or OOM in any of them.

---

## Experiments as run

The configurations form a grid: 12 per-transaction costs, and for each cost,
a ladder of `TotalComputationUnits` limits.

89 of the 137 configurations give every transaction in a run the same cost,
and 48 configurations mix two or three costs in one commit.

- **In the 89 fixed-cost configurations**, the two modes cannot produce
  different numbers: with one cost, a unit limit of `10 × cost` admits exactly
  the ten transactions a count limit of 10 admits, and any other unit limit has
  a matching count limit. However, these configurations were run to establish
  how a limit behaves on its own — what it admits, where the excess goes, how
  latency follows (findings 1, 3–6) — and, at the matching limit, that the two
  runs give the same numbers, as they must (finding 2).
- **In the 48 mixed-cost configurations**, the two modes can produce different
  numbers, only when one commit carries transactions of different cost
  (finding 6).

[`matrix.sh`](matrix.sh) runs every configuration: it calls [`run.sh`](run.sh),
which bootstraps a fresh network, runs A, resets to the same genesis with empty
databases, runs B, and scrapes Prometheus into one JSON per run. The settings:

- **Workload**: `slow::slow(n, size)` with `size = 100`, shared-object form
  via `SLOW_SHARED=true`.
  - The shared-object workload publishes one `slow::Obj` and every transaction
    takes it as a mutable input, so all of them contend on the same object and
    go through per-object congestion control. The input is not read by the
    Move code, so the cost is the same as the owned-object form measured in
    [`probe-test.md`](probe-test.md).
  - The [plan](../stress-plan.md#w1---shared-counter-contention-baseline)
    names W1 (`--shared-counter`) as H2's workload; it is not run separately,
    because with one counter every transaction costs the 1,000 CU floor on one
    hot object, which is what the `cu1k` configurations already measure.
- **Cost points**: `slow_n` determines attested CUs per transaction, measured
  by the probe and identical on both machines.
  - The cost points table:

  | Label | `slow_n` | CUs per tx | | Label | `slow_n` | CUs per tx |
  | --- | --- | --- | --- | --- | --- | --- |
  | `cu1k` | 1 | 1,000 | | `cu100k` | 350 | 100,000 |
  | `cu2k` | 70 | 2,000 | | `cu200k` | 516 | 200,000 |
  | `cu5k` | 120 | 5,000 | | `cu500k` | 1,015 | 500,000 |
  | `cu10k` | 160 | 10,000 | | `cu1m` | 1,848 | 1,000,000 |
  | `cu20k` | 217 | 20,000 | | `cu2m` | 3,511 | 2,000,000 |
  | `cu50k` | 267 | 50,000 | | `cu5m` | 8,000 | 5,000,000 |

  - The last label (`cu5m`) corresponds to the gas metering ceiling: 5,000,000
    CUs is the maximum a transaction can be charged, whatever budget it
    declares, so those transactions fail with `InsufficientGas` and are charged
    the whole budget. `succ_tps` counts them like any other executed
    transaction, which matters when reading `cu5m`'s throughput.
- **Mixed-cost configurations**: `SLOW_MIX` draws each transaction's `slow_n`
  from two or three costs in fixed proportions, written `n:weight`, so one
  commit holds transactions of different costs.
  - Example: `1:9,350:1` draws `slow_n = 1` nine times in ten and `slow_n = 350`
    once, which at `size = 100` is 1,000 CUs against 100,000.
  - Configurations are labeled as `mix<mean cost>-w<expensive %>`: for example,
    `mix10900-w10` implies mean 10,900 CUs with 10 % of transactions expensive.
  - Each mix is compared with the fixed-cost configuration of the same mean.
  - The mixed-cost configurations table:

  | Mix | Costs | Share of each | | Mix | Costs | Share of each |
  | --- | --- | --- | --- | --- | --- | --- |
  | `mix1900` | 1K / 10K | 90 / 10 % | | `mix20800` | 1K / 100K | 80 / 20 % |
  | `mix3700` | 1K / 10K | 70 / 30 % | | `mix30700` | 1K / 100K | 70 / 30 % |
  | `mix9500` | 5K / 50K | 90 / 10 % | | `mix50500` | 1K / 100K | 50 / 50 % |
  | `mix10900` | 1K / 100K | 90 / 10 % | | `mix50900` | 1K / 500K | 90 / 10 % |
  | `mix11800` | 2K / 100K | 90 / 10 % | | `mix54500` | 5K / 500K | 90 / 10 % |
  | `mix13600` | 1K / 10K / 100K | 60 / 30 / 10 % | | `mix68000` | 20K / 500K | 90 / 10 % |

- **Run A**: `TotalTxCount`, limit 10, overshoot 0.
  - Production runs this mode with limit 10 and an overshoot of 100 (protocol
    version 22 and later).
  - However, the overshoot is off in the main grid, in both runs, so that each
    run is described by one number and no debt is carried between commits.
  - Three mixed-cost configurations were then rerun with the overshoot on, and
    nothing changed (finding 6).
- **Run B**: `TotalComputationUnits`, `LIMIT_B` in CUs and overshoot 0.
  - Each of the twelve costs above was run against several `LIMIT_B` values,
    drawn from one sequence — 10,000, 20,000, 50,000, 100,000, 200,000 and so
    on.
  - The smallest unit limit is below one transaction's cost, so it admits
    nothing; the largest is 100 times the cost, capped at 50,000,000.
  - Each cost was run at four to eight of these values, 80 configurations in
    total, and the one at `10 × cost` fits exactly ten transactions per commit
    — the count limit's equivalent.
  - At `cu20k`, 20,000 CUs per transaction, the values are 10,000 (admits
    none), 20,000 (one), 50,000 (two), 100,000 (five), 200,000 (ten, the count
    limit's equivalent), and on to 2,000,000 (a hundred).
- **Both runs**: attestation on, `max_deferral_rounds = 10`, 4 validators.
- **Client**: via the fullnode (`DIRECT=false`).
  - Target QPS of 1,000 tx/s for 60 s.
  - 24 workers on 12 threads, 4 gas accounts.
  - At most `2 × 1000 = 2,000` transactions in flight: a worker submits a new
    transaction only when one of its in-flight transactions has completed.
- **Rate**: 1,000 tx/s for the main grid.
  - 2,000 tx/s for 14 configurations rerun to check it.
  - At 1,000 CUs, the limit is already what holds throughput down at the lower
    rate.
  - From 5,000 CUs up, the client's in-flight cap lowers what it offers to what
    can execute on the object, so for the heavy fixed-cost configurations, a
    higher target changes little (finding 3). For the mixes, where most
    transactions are cheap, it changes a lot: several configurations turn out
    to have been limited by the client rather than by the limit at 1,000 tx/s
    (finding 6).
- **Machines**: the grid ran on one AMD EPYC 9454P server (48 cores / 96
  threads, 251 GiB RAM, Ubuntu 24.04), running the private network in docker — 4
  validators plus 1 fullnode — with the stress client on the same host.
  - The three mix ladders were also run on the second machine of
    [`probe-test.md`](probe-test.md), a Ryzen 9 9950X3D workstation (WS below),
    which executes the same transactions about 4.6× faster.
  - Those runs carry `-ws` labels and are kept under `results/matrix-ws/`, so
    they never mix with the EPYC data.
- **137 configurations**:
  - 80 fixed-cost configurations, 10 iterations each.
  - 5 mixed-cost configurations at `10 × mean cost`, 11 iterations each — one
    run first to check the configuration, then ten more.
  - 18 more mixed-cost configurations, 10 iterations each.
  - 4 mixed-cost configurations of 300 s duration, 10 iterations each.
  - 3 mixed-cost configurations with the overshoot on, 10 iterations each.
  - 14 configurations at 2,000 tx/s — 9 fixed-cost and 5 mixed — 10 iterations
    each.
  - The WS adds 13 mixed-cost configurations, 5 iterations each.

**Aggregation and reporting tooling**:

- [`aggregate.py`](aggregate.py) — pools every label's iterations into one
  A-vs-B row per configuration, and generates `results/matrix/summary.md` for
  reading and `results/matrix/summary.csv` for plotting.
  - Rates are averaged over the iterations. For quantiles, every iteration's
    histogram counts are added together first and the quantile is computed from
    the combined histogram, rather than averaging each iteration's own quantile.
  - It also writes the standard deviation of each value across the iterations,
    the rate of transactions executed at the expensive cost of a mix, the
    per-commit admission histogram (`admits_hist.csv`) and the checkpoint lag
    per 10 s and 60 s slice of the window (`lag_over_time.csv`).
- [`plot.py`](plot.py) — renders the figures into `results/matrix/summary_plots/`;
  with `results/matrix-ws` as a second argument, it also draws the data from
  the two machines together.
- Shared with H1: [`../aggregate.py`](../aggregate.py),
  [`../dump_timeseries.py`](../dump_timeseries.py) and [`../exp_dir.py`](../exp_dir.py).

> [!NOTE]
> The deferral limit is counted in leader rounds, not scheduling attempts. A
> skipped leader round uses up a round without a retry, so 2–4 % of deferred
> transactions — 9–10 % at the heaviest cost — are cancelled after 11–12
> rounds instead of the configured 10, in both modes alike.
<!-- -->
> [!NOTE]
> The stress plan names `transactions_included_in_checkpoint` as H2's throughput
> metric. It is reported here as the finalized rate, but the headline throughput
> is **success tps = executed − cancelled − commit prologues**: user
> transactions that did real work. Checkpoint inclusion trails execution by the
> checkpoint lag, and once that checkpoint lag approaches the 60 s window, it
> undercounts what the window processed — at the three heaviest cost points,
> `included − cancelled − commit prologues` comes out at 0.0, −13.4 and −27.8
> tx/s. Cancelled transactions are subtracted because they execute but do
> nothing, and the commit rate because every commit carries one consensus commit
> prologue, a system transaction that both counters count.

---

## Findings

Numbers below are means over all iterations; latencies are exact histogram means
or quantiles over buckets combined across the 4 validators and all iterations.
Run A is set up identically in every configuration of a cost point, so its
spread across those configurations shows how much two identical runs differ:
within ±3 % of the mean everywhere except `cu1k` (±3.4 %) and `cu2k` (156–176
tx/s, ±8.8 %). Within one configuration, the standard deviation of success
throughput across its iterations is 1–4 % of the mean at most cost points, up
to 12 % at `cu1k`, 14 % at `cu5m`, and 27 % at `cu2k`, where the offered load
varies most; the figures draw it as error bars.

> [!NOTE]
> Keep the client's in-flight cap and the 60 s window in mind when reading the
> heavy-cost numbers:
>
> - **The offered load is not 1,000 tx/s.** The client keeps at most 2,000
>   transactions in flight and waits for each to finish, so it can offer only
>   2,000 divided by the current latency. At 1,000 CUs, that is the full target;
>   at 5,000 CUs, about 200 tx/s arrive; at 500,000 CUs, about 80 tx/s; at
>   5,000,000 CUs, about 40 tx/s (finding 3).
> - **Checkpoint lag is measured only for checkpoints built inside the window.**
>   A backlog that outlives the 60 s run is never recorded, so the slowest
>   checkpoints are missing from the average and the checkpoint lag mean becomes
>   lower than the truth. The bigger the backlog, the further the mean falls
>   below the truth. The share of checkpoints past 30 s is the reliable measure
>   of the tail. A quantile beyond 30 s is not, because the histogram's buckets
>   there are 30, 60, 90 and then unbounded, so the value is interpolated across
>   a 30-second gap rather than observed — the tables print `>30` instead.

In the figures, one curve corresponds to one cost point, coloured from light to
dark by cost and, where a dozen of them share a panel, given its own marker;
Run A is drawn as a star or a vertical line, since its admitted rate is the
same (to a few percent) in every configuration of a point.

---

### Fixed-cost experiments

#### 1. Both limits enforce what they are set to, and a limit below one transaction's cost admits nothing.

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `consensus_handler_scheduled_transactions_per_object_per_commit` | Number of transactions admitted (scheduled) to a shared object in a single consensus commit (one observation per object per commit) | Histogram; per-commit mean `Δ_sum / Δ_count` combined across validators over all iterations. Observed only on commits that scheduled at least one transaction on the object |
| `consensus_handler_cancelled_transactions` | Number of transactions cancelled by consensus handler | Counter; rate, averaged across validators and iterations |
| `consensus_committed_subdags` | Number of committed subdags, sliced by leader | Counter; rate equals consensus commits per second, averaged across validators |

</details>

`admits/cmt` is the measured number of transactions admitted per commit.

- Run A's limit is 10 per commit, and it admits all 10 when enough are waiting:
  9.98–10.00 across every `cu1k` and `cu2k` configuration. From `cu5k` up it
  admits less — 7.4 per commit at `cu5k`, 3.8 at `cu1m`, 4.8 at `cu5m` —
  although dozens sit deferred each commit. Admitting less than the limit
  allows is finding 3, and its cause is open.
- Run B admits `LIMIT_B / cost`, rounded down, because a transaction is admitted
  whole or not at all: a limit of 50,000 CUs admits exactly 2.000 transactions
  of 20,000 CUs; a third transaction would need a limit of at least 60,000 CUs.
  That holds in all 25 configurations where the limit has room for no more than
  what can execute on the object per commit:

| Cost point | `LIMIT_B / cost` → measured B admits/cmt |
| --- | --- |
| `cu1k` | 10 → 9.99, 20 → 19.95, 50 → 49.65 |
| `cu2k` | 5 → 5.00, 10 → 10.00 |
| `cu5k` | 2 → 2.000, 4 → 4.000 |
| `cu10k` | 1 → 1.000, 2 → 2.000, 5 → 5.000 |
| `cu20k` | 1 → 1.000, 2.5 → 2.000, 5 → 4.97 |
| `cu50k` | 1 → 1.000, 2 → 2.000, 4 → 3.999 |
| `cu100k` | 1 → 1.000, 2 → 2.000 |
| `cu200k` | 1 → 1.000, 2.5 → 2.000 |
| `cu500k` | 1 → 1.000, 2 → 2.000 |
| `cu1m`, `cu2m`, `cu5m` | 1 → 1.000 |

The worst deviation is 0.69 % (`cu1k-lim50k`, 49.65 for 50), and there the limit
is not what holds admission down: the client offers 46 transactions per commit
against room for 50. Most are exact to three decimals.

A limit below the cost of a single transaction admits nothing at all. The
scheduler's rule is `start_time + cost <= limit` with a start time of at least
0, so in the 8 configurations whose limit is under one transaction's cost, Run B
cancels everything that arrives after ten rounds — 740–950 per second in six of
the eight, fewer in the two heaviest for the reason below — and its success tps
is 0.4–1.7. Run A in the same configuration is unaffected. Once the limit allows
more than can execute on the object per commit, it stops deciding anything and
Run B's admits track Run A's instead (finding 3).

> [!NOTE]
> Two of those eight configurations show something else. In `cu2m-lim1m` and
> `cu5m-lim2m`, Run B's consensus handler could not keep up with the rate
> consensus produced commits, processing only 11.6 and 7.2 commits per second
> against Run A's 19.7 and 18.5, with skipped leader rounds climbing to 74–77
> per run from Run A's 24–45. Every transaction in those runs is deferred and
> re-evaluated every commit at 2–5 million CUs each. It is the only place in
> the grid where the mode changed the commit rate. Why the handler could not
> keep up is left for a follow-up.

---

#### 2. At the matched limit, the two modes produce the same numbers, at every cost from 1,000 to 5,000,000 CUs.

The 12 configurations where `LIMIT_B = 10 × cost` are the unit limit equivalent
of Run A's count limit of 10. In every one of them, the two runs admit the same
number of transactions per commit and achieve the same throughput:

| Cost point | Admits/cmt A → B | Success tps A → B | Success tps B/A | Cancelled/s A → B | Ckpt lag mean s A → B |
| --- | --- | --- | --- | --- | --- |
| `cu1k` | 10.00 → 9.99 | 192.7 → 193.2 | 1.003 | 753 → 745 | 0.37 → 0.41 |
| `cu2k` | 9.99 → 10.00 | 176.2 → 176.4 | 1.001 | 226 → 234 | 4.13 → 3.97 |
| `cu5k` | 7.44 → 7.41 | 117.9 → 117.4 | 0.995 | 61 → 61 | 11.3 → 11.4 |
| `cu10k` | 6.47 → 6.38 | 93.6 → 94.5 | 1.009 | 44 → 45 | 14.0 → 14.7 |
| `cu20k` | 5.60 → 5.60 | 73.7 → 74.2 | 1.006 | 38 → 38 | 17.3 → 17.2 |
| `cu50k` | 5.21 → 5.11 | 63.7 → 62.7 | 0.984 | 34 → 34 | 18.5 → 18.7 |
| `cu100k` | 4.73 → 4.79 | 51.1 → 52.0 | 1.016 | 31 → 31 | 23.2 → 22.7 |
| `cu200k` | 4.51 → 4.51 | 38.8 → 39.3 | 1.013 | 26 → 26 | 19.6 → 19.1 |
| `cu500k` | 4.05 → 4.04 | 24.1 → 24.2 | 1.003 | 25 → 25 | 22.1 → 21.9 |
| `cu1m` | 3.78 → 3.77 | 15.5 → 15.6 | 1.004 | 26 → 25 | 29.0 → 29.7 |
| `cu2m` | 4.33 → 4.38 | 8.4 → 8.4 | 1.002 | 29 → 28 | 30.0 → 32.1 |
| `cu5m` | 4.83 → 4.67 | 3.2 → 3.3 | 1.025 | 17 → 17 | 19.7 → 20.5 |

- Success tps B/A averages 1.005 over the twelve, between 0.984 and 1.025 —
  closer to 1 than the standard deviation between iterations of a single run, so
  none of the ratios is distinguishable from 1. Settlement latency matches at
  p95 and to within 11 % at the median (finding 5).
- *This is the expected result, and it is what lets the rest of the grid be read
  as evidence: a limit in CUs and a limit in transaction count are
  interchangeable at uniform cost, so any difference in a mixed-cost
  configuration comes from the cost spread and nothing else.*
- The other 68 configurations vary `LIMIT_B` away from that equivalence, and
  are read in findings 3 and 4.

![Run A against Run B at the twelve fixed-cost matched configurations](results/matrix/summary_plots/modes_matched.png)

*The twelve fixed-cost matched configurations, `LIMIT_A = 10`,
`LIMIT_B = 10 × cost`: Run A (star) next to Run B (dot), with one standard
deviation across iterations as error bars. The number above each pair is B/A
on that panel's own metric — throughput, cancellations, or checkpoint lag.*

![Success tps, cancelled fraction, checkpoint lag and its share over 30 s for
Run A and every Run B limit at each cost
point](results/matrix/summary_plots/modes_heatmaps.png)

*Every configuration at a glance: success tps, cancelled fraction of offered,
checkpoint lag mean, and the share of checkpoint lag over 30 s, one panel each.
Run A is the left column and each `LIMIT_B` follows to its right, with the cost
points down the side. Within a panel, the same colour means the same value;
success tps and checkpoint lag use a log colour scale. The outlined cells are
the `10 × cost` limit, the one that matches Run A: in the success tps panel they
read 193 against Run A's 195 at `cu1k`, 176 against 172 at `cu2k`, and so on
down the column.*

---

#### 3. Throughput is set by how fast transactions on one object execute, one after another. From 2,000 CUs up, each limit admits more transactions per second than can be executed on the object.

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `execution_driver_executed_transactions` | Cumulative number of transactions executed by execution driver | Counter; rate, averaged across validators and iterations. Counts cancelled transactions and the per-commit system transaction too, so success tps subtracts both |
| `transactions_included_in_checkpoint` | Transactions included in a checkpoint | Counter; rate, averaged across validators — the finalized rate, prologues included |
| `consensus_handler_deferred_transactions` | Number of transactions deferred by consensus handler | Counter; rate. A transaction counts once for every commit it stays deferred |

</details>

Transactions on one mutable shared object execute one after another, so at each
cost, there is a top rate at which they complete — **the drain rate** in the
table. It is measured in the configurations whose limit lets in more than can be
executed on the object: there the limit is not what holds throughput down, so
success tps settles at that rate and does not rise as the limit goes higher. The
rate at 5,000,000 CUs is nearly 50 times lower than at 2,000, while the count
limit stays at 10 per commit throughout:

| Cost point | CUs per tx | Drain rate (tx/s) | Arrivals/s | Scheduled/s | Success tps | Scheduled / success tps |
| --- | --- | --- | --- | --- | --- | --- |
| `cu1k` | 1,000 | Not reached (the client offers too little) | 944 | 191 | 192.7 | 0.99 |
| `cu2k` | 2,000 | 179 | 425 | 199 | 176.2 | 1.13 |
| `cu5k` | 5,000 | 118 | 206 | 145 | 117.9 | 1.23 |
| `cu10k` | 10,000 | 94 | 164 | 120 | 93.6 | 1.28 |
| `cu20k` | 20,000 | 73 | 141 | 103 | 73.7 | 1.40 |
| `cu50k` | 50,000 | 62 | 128 | 94 | 63.7 | 1.47 |
| `cu100k` | 100,000 | 52 | 114 | 82 | 51.1 | 1.61 |
| `cu200k` | 200,000 | 39 | 96 | 70 | 38.8 | 1.80 |
| `cu500k` | 500,000 | 24 | 81 | 56 | 24.1 | 2.31 |
| `cu1m` | 1,000,000 | 15 | 72 | 46 | 15.5 | 2.96 |
| `cu2m` | 2,000,000 | 8.6 | 65 | 36 | 8.4 | 4.30 |
| `cu5m` | 5,000,000 | 3.8 | 40 | 23 | 3.2 | 7.12 |

Each row is Run A — the count limit of 10 — taken from the configuration whose
`LIMIT_B` is `10 × cost`, the unit limit that allows the same ten transactions.
Run B of those same configurations gives the same numbers, to within 2.5 %.
Every rate is the mean over the ten iterations, and within an iteration, the
mean over the four validators. The drain rate is the median of the success tps
described above. Scheduled is how many transactions per second the scheduler let
onto the object, the admits histogram's `_sum` over time; arrivals is scheduled
plus cancelled, everything the client got into consensus.

*The offered load drops as cost rises.* The client can only offer 2,000
transactions divided by how long each takes:

- at 1,000 CUs, the full 1,000 tx/s target (944 arrive);
- at 5,000 CUs, about 200 tx/s, because a transaction takes about 10 s from
  submission to result, 6.8 s of it settlement;
- at 5,000,000 CUs, 40 tx/s.

So the configurations from `cu2k` up do not measure the network under a
1,000 tx/s load. They measure it under whatever load its own latency lets
through. The target rate describes the lightest cost point only: at `cu2k`,
the client already delivers 425 tx/s of the 1,000.

*The scheduler admits more than executes on the object.* Scheduled exceeds
success tps by 13 % at 2,000 CUs and by 7× at 5,000,000 — every scheduled
transaction beyond the drain rate joins an execution backlog that grows for the
whole window. Both limits let this happen because neither is a measure of time:
a count of 10 is the same ten transactions whether each takes 0.6 ms or 310 ms,
and `10 × cost` units is the same amount of "work" whether the machine executes
a unit in a nanosecond or a microsecond. The backlog is what checkpoint lag
measures (finding 4).

*Under overload, both modes admit fewer transactions than their limit
allows.* In the commits where the object received anything:

- Run A admitted 7.4 per commit at `cu5k` and 3.8 at `cu1m`, against a limit
  of 10.
- Dozens of deferred transactions sat queued, 57 per commit at `cu5k`, and a
  quarter of commits admitted only 2–5.
- The share of commits that scheduled anything at all falls from 96 % at
  `cu1k` to 26 % at `cu5m`.

Run B at the matched limit shows the same numbers, so this does not affect the
A-vs-B comparison. But it means the scheduler leaves capacity unused while
transactions wait and are later cancelled.

> [!NOTE]
> Why it happens is not yet known. Debt carried between commits cannot explain
> it, because with overshoot 0 there is none. Worth a dedicated look.

![Checkpoint lag and cancelled fraction of offered against what the limit
allows](results/matrix/summary_plots/modes_admitted_rate.png)

*Checkpoint lag (top) and cancelled fraction of offered (bottom) against what
the limit allows as a rate — `LIMIT_B ÷ cost` per commit × commits/s — one
curve per transaction cost point; the dashed line is Run A, whose 10
transactions per commit is ≈200 tx/s at 20 commits/s. Left of the dashed line,
the curves are still moving — checkpoint lag climbing, cancellations falling.
From it rightward, they mostly flatten: past that rate, the scheduler takes in
nearly everything that arrives, so the limit is no longer what caps admission.
Raising the limit only removes the last cancellations; checkpoint lag stays
high, and at some costs drifts up further. `cu1k` is the exception, where the
client cannot offer enough to get there.*

---

#### 4. The limit determines where the excess lands: cancelled by the scheduler, or queued for execution.

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `checkpoint_creation_latency` | Latency from consensus commit timestamp to local checkpoint creation in milliseconds | Histogram; the exact mean from `Δ_sum / Δ_count` and the share of observations above the 30 s bucket edge, combined across validators over all iterations. Quantiles past 30 s print as `>30` |

</details>

Every cost point shows the same two stages as the limit is raised:

- While the limit admits fewer transactions than can be executed on the object,
  the excess is deferred and cancelled at ten rounds: 830–960/s, most of what
  arrives. Checkpoint lag stays at 1.5 s or below, because nothing scheduled is
  waiting to execute.
- Once the limit admits more than can be executed on the object, the excess is
  queued instead of cancelled. Cancellations fall towards zero, success tps
  flattens at the drain rate, and checkpoint lag jumps by five to forty times
  and goes on rising as the limit loosens:

| Cost point | Drain rate (tx/s) | Last limit that still cancels: admitted/s → checkpoint lag s, cancelled/s | First limit that queues instead: admitted/s → checkpoint lag s, cancelled/s |
| --- | --- | --- | --- |
| `cu2k` | 179 | 100 → 0.3, 877 | 200 → 4.0, 234 |
| `cu5k` | 118 | 80 → 0.4, 904 | 148 → 11.4, 60 |
| `cu10k` | 94 | 40 → 0.3, 942 | 100 → 3.5, 372 |
| `cu20k` | 73 | 40 → 0.5, 923 | 99 → 7.9, 175 |
| `cu50k` | 62 | 40 → 1.3, 852 | 80 → 6.2, 223 |
| `cu100k` | 52 | 40 → 0.5, 924 | 88 → 13.3, 89 |
| `cu200k` | 39 | 40 → 1.4, 767 | 79 → 17.3, 64 |
| `cu500k` | 24 | 20 → 0.3, 957 | 40 → 12.8, 140 |
| `cu1m` | 15 | — | 20 → 9.3, 196 |
| `cu2m` | 8.6 | — | 20 → 16.5, 102 |
| `cu5m` | 3.8 | — | 18 → 18.5, 56 |

At `cu1m`, `cu2m`, and `cu5m`, the left column is empty because the only
smaller limit on their ladders admits nothing at all: 500K, 1M, and 2M CUs are
each less than one transaction of that cost, and a transaction is admitted
whole or not at all, so nothing is ever scheduled on the object and what
arrives is cancelled instead. The next step up — a limit equal to one
transaction's cost — already admits one per commit, 18–20 tx/s at 20 commits a
second, against drain rates of 15, 8.6 and 3.8. At those costs, every limit
that admits anything builds a backlog.

Once the excess is queued, raising the limit further adds no throughput, as a
row of the heatmaps in finding 2 shows:

- at `cu50k`, the limits admitting 4 to 100 transactions per commit all have a
  success tps of 60–64, while the checkpoint lag mean climbs from 6 s to 27 s
  and the share of checkpoints past 30 s from 0 to 30 %;
- at `cu100k` and `cu200k`, the top limits put 85–90 % of checkpoints past 30 s,
  and their checkpoint lag means of 33–42 s are lower bounds (see the note on
  the 60 s window above).

> [!IMPORTANT]
> The limit only sets where the excess lands, not how much gets executed. A
> tight limit turns the excess into cancellations, which the client sees as
> failures within ten rounds and can resubmit. A loose limit turns the excess
> into a queue, which the client sees as latency and the network as checkpoint
> lag. Success tps is the same either way once execution on the object
> is saturated, so no setting of either mode raises it. The only choice is
> which failure the users get.

![Checkpoint lag mean against success tps per configuration](results/matrix/summary_plots/modes_tradeoff.png)

*Checkpoint lag mean against success tps, one point per configuration: dots
are Run B, growing with the limit up to the labelled largest one; stars — Run
A. The lower right corner of each panel is where high success tps meets low
checkpoint lag; as the limit loosens, each cost's points first move right along
the bottom, then climb straight up: no extra throughput, but more checkpoint
lag.*

![Checkpoint lag mean and cancelled fraction of offered against the rate the
limit allows divided by each cost's drain
rate](results/matrix/summary_plots/modes_utilization.png)

*Checkpoint lag mean (left) and cancelled fraction of offered (right) against
the rate the limit allows, divided by each cost point's drain rate — finding
3's figure with its x-axis rescaled. The cancelled fraction curves fall onto
almost one line and drop to near zero just past the dashed line, where a limit
allows exactly the drain rate: from there on, the excess is queued instead of
cancelled. The checkpoint lag curves rise at the same place but settle at
different heights: 9 s at `cu2k` to 41 s at `cu200k`.*

The figure below plots cancellations against checkpoint lag for every fixed-cost
configuration. These two measurements move in opposite directions: a limit
either cancels the excess or queues it, so no limit reaches the bottom left
corner, where both would be low. The one exception is `cu1k`: at 1,000 CUs,
the client cannot offer enough to saturate the object, and its loosest limits
cancel nothing at 0.2–1.0 s of checkpoint lag.

The unfilled markers in the figure below are limits for which another limit at
the same cost point is no worse on either count and better on at least one.
There are 23 of such markers across all cost points except `cu50k`, and they
fall into two groups:

- Up to `cu200k`, eleven of the twelve are also beaten at the end of the run. At
  `cu1k`, the object is never saturated, so the loosest limit lets everything in
  and cancels nothing. Seven of the other eight, from `cu2k` to `cu200k`, sit
  past the drain rate, where the scheduler already takes in nearly everything
  that arrives (finding 3): one limit and the next differ by the last few
  cancellations, or none, at a checkpoint lag within one standard deviation
  across iterations.
- From `cu500k` up, ten of the eleven are not beaten at the end of the run:
  there the larger limit ends with the higher checkpoint lag — at `cu500k`, 52 s
  at 50M against 40 s at 10M. Its mean is lower only because it builds far fewer
  checkpoints after the first 10 s (229 checkpoints against 3,072), so the early
  checkpoints, built before the backlog formed, make up a larger share of its
  mean. That is the lower bound described in the note on the 60 s window above.

![Cancelled fraction of offered against checkpoint lag mean, for every
fixed-cost configuration](results/matrix/summary_plots/modes_pareto_fixed.png)

*Every fixed-cost configuration, one colour and marker per cost point: the
cancelled fraction of offered against checkpoint lag mean. Run B is drawn as
dots, Run A — starred, and the line joins the limits tried at one cost point in
order. The unfilled markers are limits that another limit at the same cost point
matches or beats on both axes, though from `cu500k` up, most of them come from
how the checkpoint lag mean is weighted, not from a smaller backlog; the x-axis
is linear below 0.01 and logarithmic above, because every limit that queues
cancels under 15 % of what is offered. Success tps is not on an axis because it
trades against neither: it climbs to the drain rate as the limit grows and stays
there.*

---

#### 5. Latency is identical between modes at matched limits; set by the backlog, not the mode.

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `transaction_driver_settlement_finality_latency` | Settlement finality latency observed from transaction driver | Histogram on the fullnode (the runs submit through it); p50/p95 over buckets combined across iterations |
| `validator_transaction_execution_latency` | Validator-internal latency from receiving a transaction via `submit_tx` until it finished executing (pre-consensus check, consensus, post-consensus validation, sequencing incl. deferral, execution) | Histogram; p95 over buckets combined across validators and iterations |
| `authority_state_internal_execution_latency` | Latency of actual certificate executions | Histogram; mean, validators only. Blends the per-commit system transactions and the cancelled transactions in with the workload, so it understates the workload's cost where cancellations are many |

</details>

At the twelve matched configurations, the client-facing latency is the same in
both modes at every cost point:

| Cost point | Settlement p50 ms A → B | Settlement p95 ms A → B | Receipt→executed p95 ms A → B | VM exec mean ms A → B |
| --- | --- | --- | --- | --- |
| `cu1k` | 728 → 726 | 877 → 872 | 983 → 986 | 0.23 → 0.23 |
| `cu2k` | 788 → 789 | 7,566 → 7,291 | 7,166 → 6,852 | 2.44 → 2.39 |
| `cu5k` | 6,803 → 6,635 | 18,813 → 18,861 | 18,436 → 18,435 | 5.05 → 5.07 |
| `cu10k` | 7,844 → 8,692 | 23,448 → 23,345 | 25,555 → 24,930 | 6.34 → 6.31 |
| `cu20k` | 8,648 → 8,874 | 28,013 → 28,028 | 26,925 → 26,953 | 7.60 → 7.58 |
| `cu50k` | 6,788 → 6,437 | 27,732 → 27,818 | 28,153 → 28,395 | 8.50 → 8.57 |
| `cu100k` | 2,704 → 3,003 | 26,220 → 26,675 | 46,888 → 44,960 | 9.80 → 9.75 |
| `cu200k` | 4,034 → 4,409 | 25,691 → 25,706 | 50,365 → 50,251 | 11.8 → 11.8 |
| `cu500k` | 859 → 881 | 24,840 → 24,774 | 48,655 → 48,447 | 14.4 → 14.4 |
| `cu1m` | 1,427 → 1,411 | 25,417 → 24,278 | 46,839 → 47,819 | 16.3 → 16.5 |
| `cu2m` | 3,627 → 3,695 | 21,833 → 21,906 | 47,885 → 47,556 | 17.6 → 17.7 |
| `cu5m` | 7,773 → 7,881 | 21,290 → 21,145 | 44,723 → 44,626 | 25.4 → 25.1 |

The p95 values match to within 5 %. The medians differ by up to 11 % (`cu10k`,
`cu100k`) — that is noise: Run A's own median varies more than that across
the configurations of one cost, 7.8 to 8.9 s at `cu10k`.

Along a ladder, latency follows the backlog of finding 4 and nothing else:

- Under every limit that admits less than the drain rate, transactions settle
  in ≈ 730–750 ms at the median and ≈ 900 ms at p95, at every cost from 1,000
  to 500,000 CUs — nothing admitted waits, because the excess is cancelled
  rather than queued.
- The first limit above the drain rate takes p95 to 6–25 s while the median
  mostly holds near 750 ms: the backlog on the object is still short, so only
  a minority of admitted transactions queue behind it; most are executed as
  soon as they are admitted. At most costs, one or two steps further up the
  ladder, the median rises as well, to 3–16 s.
- p95 of the receipt-to-executed latency — the validator's own view including
  time a transaction spent being deferred — reaches 45–56 s at the heavy
  points, close to the length of the 60 s run itself.
- The settlement latency median does not fall steadily as cost rises — 8.6 s at
  `cu20k`, 0.86 s at `cu500k` — because the client's offered load shrinks with
  cost. With fewer transactions in flight, the ones that do get admitted wait
  behind fewer others.

VM execution time, the table's last column, is the same in both modes at every
cost, 0.23 to 25 ms: cost is a property of the transaction, and the mode only
decides how many of them are let in.

---

### Mixed-cost experiments

#### 6. When one commit carries transactions of different cost, the modes differ, and a well chosen unit limit does better than the count limit — on throughput, cancellations, and checkpoint lag at the same time.

The five findings above hold cost fixed within a run, where a unit limit is
only a count limit written in other units: at the matched limit, the two modes
give the same numbers, and at any other limit, they differ only in how many
transactions that limit allows. The mixed-cost (`SLOW_MIX`) configurations give
the transactions in one commit two or three costs.

##### At the count limit's equivalent

The first five mixed-cost configurations ran at `LIMIT_B = 10 × mean cost`, so
Run B's budget per commit is what ten transactions of the mix cost on average —
the work the count limit admits. What differs is how that budget is spent: Run
A always takes ten transactions, Run B — as many as the unit limit can fit,
which in a commit holding no expensive transaction is far more than ten.

> [!TIP]
> How to read a row: `mix1900` names the mix by its mean cost, 1,900 CUs per
> transaction. "1K / 10K, 9:1" gives the two costs it draws from — cheap
> transactions of 1,000 CUs and expensive ones of 10,000 CUs — and their
> proportions: nine cheap for every expensive one. The mean cost follows:
> 0.9 × 1,000 + 0.1 × 10,000 = 1,900. Its unit limit `LIMIT_B = 10 × mean cost`
> is therefore 19,000 CUs.

| Configuration | Costs (units), ratio | Admits/cmt A → B | Success tps A → B | Success tps B/A | Cancelled/s A → B | Ckpt lag mean s A → B |
| --- | --- | --- | --- | --- | --- | --- |
| `mix1900` | 1K / 10K, 9:1 | 10.00 → 13.3 | 200.9 → 265.9 | 1.32 | 793 → 719 | 0.42 → 0.44 |
| `mix3700` | 1K / 10K, 7:3 | 10.00 → 13.6 | 198.4 → 272.9 | 1.38 | 788 → 727 | 0.55 → 0.34 |
| `mix10900` | 1K / 100K, 9:1 | 10.00 → 31.5 | 199.9 → 626.7 | 3.13 | 791 → 366 | 0.78 → 0.63 |
| `mix20800` | 1K / 100K, 4:1 | 10.00 → 21.6 | 194.7 → 402.9 | 2.07 | 786 → 186 | 0.74 → 3.88 |
| `mix50900` | 1K / 500K, 9:1 | 10.0 → 14.8 | 181.9 → 265.0 | 1.46 | 788 → 31 | 2.34 → 8.06 |

- Run A admits exactly 10 in every commit — 100 % of its commits sit in the
  7–10 bucket of the admission histogram.
- Run B's limit is in CUs, so what it admits depends on what the commit holds.
- At `mix1900`, the limit is 19,000 CUs:
  - a commit holding one expensive (10,000 CUs) transaction has room for 9
    cheap ones (10 in total);
  - a commit holding no expensive transaction fits 19 cheap ones;
  - measured: 63 % of commits sit at 7–10 and 37 % — at 10–20.
- At `mix10900`, the limit is 109,000 CUs:
  - one expensive (100,000 CUs) transaction plus 9 cheap ones, or
  - 109 cheap transactions;
  - measured: 78 % of commits sit at 7–10 and 22 % — at 100–200.
- The extra admissions are cheap transactions that a count of 10 would have
  deferred and, mostly, cancelled after ten rounds.
- Cancellations fall from ≈790/s in every Run A to 719, 727, 366, 186 and 31/s
  in Run B.

##### Which unit limit is the best

Four of the mixed-cost configurations were then run at several limits. The
[stress plan](../stress-plan.md#what-we-want-to-checkconfirm) asks how the two
modes compare on throughput and latency; for the unit mode, the answer depends
on where its limit is set. Mixed cost is the only setting where the two modes
can differ:

| Configuration | Limit | Admits/cmt B/A | Success tps B/A | Cancelled/s B/A | Checkpoint lag mean s B/A | Expensive executed/s B/A |
| --- | --- | --- | --- | --- | --- | --- |
| `mix3700` (1K/10K, 7:3) | 10K | 0.54 | 0.55 | 1.10 | 1.48 | 0.17 |
|  | 20K | 0.81 | 0.82 | 1.05 | 0.38 | 0.44 |
|  | 37K | 1.36 | 1.38 | 0.92 | 0.62 | 0.87 |
|  | 100K | 1.62 | 1.29 | 0.03 | 18.01 | 1.27 |
| `mix10900` (1K/100K, 9:1) | 50K | 4.53 | 4.52 | 0.13 | 0.76 | 0.00 |
|  | 109K | 3.15 | 3.13 | 0.46 | 0.81 | 0.92 |
|  | 200K | 3.60 | 3.45 | 0.19 | 6.70 | 1.93 |
|  | 500K | 2.26 | 1.87 | 0.00 | 8.54 | 2.28 |
| `mix20800` (1K/100K, 4:1) | 100K | 2.94 | 3.02 | 0.52 | 0.35 | 0.41 |
|  | 150K | 4.10 | 4.14 | 0.23 | 0.60 | 0.58 |
|  | 208K | 2.17 | 2.07 | 0.24 | 5.24 | 0.99 |
|  | 500K | 1.37 | 1.16 | 0.01 | 13.30 | 1.21 |
| `mix50900` (1K/500K, 9:1) | 200K | 4.62 | 4.94 | 0.13 | 0.13 | 0.00 |
|  | 509K | 1.48 | 1.46 | 0.04 | 3.44 | 0.88 |
|  | 1M | 1.44 | 1.32 | 0.01 | 3.87 | 1.20 |

Every cell is Run B divided by Run A of the same configuration. Run A is the
count limit of 10 in every row; its admits, success tps, cancellations and
checkpoint lag for each mix are in the `10 × mean cost` table above, and it
executes 59, 17, 34 and 16 expensive transactions per second in the four mixes,
top to bottom. Its checkpoint lag varies from row to row (0.4–1.4 s at
`mix3700`), so the checkpoint lag ratios are the least steady column.

**The count limit's equivalent is not always the best limit.** At `mix20800`:

- Unit limit of 208K CUs (`10 × mean cost`) admits two 100K transactions per
  commit and 8 cheap ones. 88 % of Run B's commits admit 10 transactions or
  fewer, and checkpoint lag is 3.9 s.
- One step down at 150K, only one expensive transaction fits and the remaining
  50K CUs go to cheap ones. Run B admits 41 transactions per commit (50 % of
  commits sit at 20–50, 41 % — at 50–70), reaches a success tps of **821
  against Run A's 198**, cancels 180/s against 795, and its checkpoint lag is
  **0.61 s against Run A's 1.0 s**.
- At 100K, exactly one expensive transaction fits and nothing else, so commits
  alternate between one expensive transaction (71 %) and 70–100 cheap ones
  (29 %): 584 tx/s, checkpoint lag of 0.59 s.

The other two ladders show the same trade-off between success tps and checkpoint
lag. At one of them, `10 × mean cost` is the best limit, at the other — it is
not:

- `mix10900`: Unit limit of 200K CUs does better than 109K on throughput (693
  against 627) and cancellations (152 against 366). But 200K is exactly twice
  the expensive cost, so two transactions fit per commit, and checkpoint lag
  rises to 1.7 s against Run A's 0.3–0.8 s. At 500K, success tps falls to 375
  tx/s and checkpoint lag rises to 5.8 s.
- `mix3700`, where the expensive transaction costs 10K CUs: here the
  `10 × mean cost` limit is the best, at 37K CUs, three expensive transactions
  plus seven cheap ones. Unit limits of 10K and 20K fit only one or two
  expensive transactions, respectively, and fall below the count limit: 109 and
  164 tx/s against 199. Unit limit of 100K CUs fits ten expensive transactions
  and has a checkpoint lag of 7.5 s.

##### What decides the best limit

The work a commit admits must execute within the ≈50 ms before the next commit.
Transactions on the same shared object execute one after another, and consensus
commits every ≈50 ms. On the EPYC machine, a 100K CU transaction takes ≈34 ms to
execute, a 10K one — ≈16 ms, a 500K one — ≈81 ms, and a cheap one — 0.6 ms
([`probe-test.md`](probe-test.md)). Adding up what each limit admits and
comparing it with the 50 ms:

| What a commit admits | Execution time | Checkpoint lag |
| --- | --- | --- |
| One 100K tx plus fifty cheap ones | ≈62 ms | < 1s (`mix20800` at 100K and 150K) |
| Two 100K txs | 67 ms | 3.9 s (`mix20800` at 208K) |
| Three 10K txs plus seven cheap ones | 52 ms | 0.34 s (`mix3700` at 37K) |
| Ten 10K txs | 159 ms | 7.5 s (`mix3700` at 100K) |
| One 500K tx | 81 ms | 8–9 s (`mix50900` at 509K and 1M) |

The last row is why `mix50900`'s checkpoint lag is high at *every* limit that
admits an expensive transaction on the EPYC machine: 8.1 s at 509K, 9.1 s at 1M.
No unit limit fixes transactions that overrun the commit on their own, though
a faster machine can make the same ones fit (the WS ladders and results below).

The count limit has the same problem and no knob at all. At `mix20800`, its ten
admissions hold two expensive transactions on average, so Run A's checkpoint
lag grows too, for the whole of a 300 s run (see below).

> [!WARNING]
> **A limit below the expensive cost looks best only because it never admits the
> expensive transactions.** With `mix10900` at 50K CUs and `mix50900` at 200K,
> Run B cancels every expensive transaction — 100/s, the 10 % that arrive —
> while the cheap transactions alone give a success tps of 900 at 0.3–0.5 s
> checkpoint lag. Those are the fewest cancellations and the lowest checkpoint
> lag at any limit of those two mixes, and they come from never letting an
> expensive transaction through. So any rule for choosing the limit must keep
> it at or above the expensive transaction's cost: those transactions still
> have to get through.

![Success tps, cancellations and checkpoint lag against the limit, for the four mixes run at several limits](results/matrix/summary_plots/modes_mix_ladders.png)

*Each panel plots one metric of Run B (orange) at each unit limit tried, with
one standard deviation across iterations as error bars; Run A (blue dashes) is
the reference, the same at every limit, with its own standard deviation as the
blue band. The dotted line marks the expensive transaction's cost, the
dash-dotted line — `10 × mean cost`, and the shaded band marks the limits that
fit one expensive transaction per commit, not two. The bottom row is how many
expensive transactions execute per second. At 1K/100K, success tps peaks inside
the band and checkpoint lag rises at the limit that fits two expensive
transactions; at 1K/10K, it peaks at a limit of 37K CUs, which fits three
expensive transactions per commit.*

Across the four mixes run at several limits, six unit limits beat the count
limit on success tps, cancellations, and checkpoint lag at the same time: 37K
CUs at `mix3700`, 50K and 109K at `mix10900`, 100K and 150K at `mix20800`, and
200K at `mix50900`. None of them also executes more expensive transactions than
the count limit does, and two of the six execute none at all — the case the
warning above describes. Whether that is an acceptable trade is a question the
data cannot answer.

![The four ladders against their own count limit, on success tps, cancellations,
checkpoint lag and expensive transactions
executed](results/matrix/summary_plots/modes_pareto_mix.png)

*Each unit limit divided by the count limit of its own mix, so Run A is the
star at (1, 1) in all three panels. Success tps B/A is on the x axis; the
y axes are checkpoint lag, cancellations, and expensive transactions executed
per second. The shaded quadrant is where Run B is better on both axes of that
panel, the dashed line joins the configurations no other configuration beats on
both, and the grey dots are the mixes that ran at one limit only. The six
ringed configurations are in the shaded quadrant of the first two panels; in
the third, they all sit below 1.*

##### Which workloads leave the unit limit anything to gain

The gain in success tps disappears once the cheap transactions are no longer
cheap. Three sets of mixes hold the expensive transactions fixed and make the
cheap ones costlier:

| Mixes | Cost of the cheap transactions | Success tps B/A |
| --- | --- | --- |
| `mix50900` → `mix54500` → `mix68000` | 1K → 5K → 20K | 1.46× → 1.10× → 1.07× |
| `mix10900` → `mix11800` | 1K → 2K | 3.13× → 1.05× |
| `mix9500` | 5K | 1.04× |

In those configurations, both runs' admission histograms sit below 10 and Run
A's checkpoint lag is already 9–23 s. Execution on the object is kept saturated
by the cheap transactions alone — 5K transactions drain at 118/s, 2K — at 179/s
(finding 3) — so admitting more of them buys nothing.

So the success tps gain does not come from the gap between the two costs. It
comes from whether Run A's ten transactions leave the object idle, because only
then does Run B's limit have room to add more. On EPYC, that needs cheap
transactions of ≈1,000 CUs: at 2K or 5K, ten of them already saturate the
object (finding 3).

Raising the share of expensive transactions removes the success tps gain too,
and for the same reason: past 20 %, the expensive transactions alone saturate
the object:

| Mix | Expensive share | Success tps B/A | Checkpoint lag A → B |
| --- | --- | --- | --- |
| `mix10900` | 10 % | 3.1× | 0.8 → 0.6 s |
| `mix20800` | 20 % | 2.1× | 0.7 → 3.9 s |
| `mix30700` | 30 % | 1.4× | 7.3 → 9.7 s |
| `mix50500` | 50 % | 1.3× | 15.3 → 15.6 s |

The three-cost mix (`mix13600`, 1K/10K/100K at 60/30/10 %) behaves the same
way: success tps B/A is 1.17 and checkpoint lag goes from 5.5 s in Run A to
9.7 s in Run B, because its middle transactions of 10K CUs take the capacity
the cheap ones would have filled.

![Admitted per commit, and the outcome, for the mixes at the count limit's equivalent](results/matrix/summary_plots/modes_mix.png)

*The twelve mixes run at `LIMIT_B = 10 × mean cost`. The top two rows are the
admission histograms: for each bucket of transactions admitted per commit, the
share of the commits that scheduled anything on the object, Run A (blue) next
to Run B (orange). Run A fills the 7–10 bucket wherever ten transactions are
waiting — 99–100 % of commits in six of the mixes, and as little as 33 % where
the object is already saturated — while Run B spreads across buckets, because
how many transactions can fit under its limit depends on what the commit holds.
The bottom row gives each mix's success tps, cancellations, and checkpoint lag,
and it follows the histograms: success tps B/A stays within 6 % of admits B/A
in all twelve, from 1.04 at `mix9500` to 3.13 at `mix10900`.*

##### Checkpoint lag in five-minute runs

A 60 s run cannot distinguish a queue that has settled from one that is still
growing, so four configurations ran for 300 s, ten iterations each: the two
recommended limits (`mix20800` at 100K and 150K), the count limit's equivalent
at `mix10900` (109K), and the limit at exactly twice the expensive cost, also at
`mix10900` (200K). Checkpoint lag per 60 s slice of the five-minute window,
the exact mean over the checkpoints built in that slice across the iterations:

| Configuration | Run | 0–60 s | 60–120 | 120–180 | 180–240 | 240–300 | Success tps |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `mix10900` at 109K | A | 0.86 | 1.04 | 0.90 | 0.41 | 0.41 | 200 |
| | B | 0.30 | 0.46 | 0.85 | 0.34 | 0.73 | 631 |
| `mix20800` at 100K | A | 0.60 | 2.02 | 3.69 | 4.98 | 6.28 | 186 |
| | B | 0.69 | 1.48 | 0.56 | 0.40 | 0.67 | 590 |
| `mix20800` at 150K | A | 0.68 | 2.27 | 4.50 | 5.76 | 7.33 | 181 |
| | B | 0.54 | 0.41 | 0.63 | 0.30 | 0.37 | 822 |
| `mix10900` at 200K | A | 1.00 | 1.65 | 0.85 | 1.09 | 0.53 | 199 |
| | B | 1.45 | 2.58 | 3.66 | 4.69 | 5.54 | 702 |

Success tps over 300 s matches the 60 s runs to within 2 %, except Run A at
`mix20800`, where the growing backlog costs it 4–9 %. Checkpoint lag does not
match: in three of the four configurations, it climbs in one of the two runs for
the whole five minutes without levelling off. The run in which checkpoint
lag climbs is predicted by the same rule as in the rest of this finding — the
work a commit admits against the ≈50 ms between commits, at ≈34 ms per 100K
transaction and 0.6 ms per cheap one:

| Configuration | Run | Admits/cmt | Expensive per cmt | Work per commit | Over 300 s |
| --- | --- | --- | --- | --- | --- |
| `mix10900` at 109K | A | 10.0 | 0.84 | 34 ms | Flat |
| | B | 31.5 | 0.78 | 44 ms | Flat |
| `mix20800` at 100K | A | 10.0 | 1.73 | 63 ms | 0.6 → 6.3 s |
| | B | 29.5 | 0.71 | 40 ms | Flat |
| `mix20800` at 150K | A | 10.0 | 1.78 | 65 ms | 0.7 → 7.3 s |
| | B | 41.0 | 1.00 | 56 ms | Flat |
| `mix10900` at 200K | A | 10.0 | 0.86 | 34 ms | Flat |
| | B | 36.5 | 1.58 | 73 ms | 1.5 → 5.5 s |

The 300 s runs show one limit holding up and another one failing:

- The recommended limit holds over five minutes. With `mix20800` at 150K CUs,
  Run B's checkpoint lag is 0.3–0.6 s in every one of the five minutes, while in
  Run A (the count limit), it climbs to 7.3 s. The 150K unit limit is the one
  that already gave 4.1× the success tps and under a quarter of the
  cancellations in the 60 s runs. *So at this mix, the unit limit is not only
  faster and cancelling less, it is the run that stays stable, and the count
  limit is the one that does not.*
- The limit at exactly twice the expensive cost is worse than 60 s made it look.
  In the 60 s run, `mix10900` at 200K CUs has a mean checkpoint lag of 1.7 s,
  low enough to look like a working limit. Over 300 s, its checkpoint lag climbs
  to 5.5 s in the last minute and is still climbing at the end. Its throughput
  advantage over the 109K CUs limit (693 against 627) is a backlog being built
  and not capacity being used. That settles the question left open in the ladder
  table above: *the 200K CUs limit is not a better limit than 109K, it is an
  unstable one, and the upper bound of the rule holds: a unit limit must stay
  below twice the expensive cost, because at twice that cost two expensive
  transactions fit in one commit — ≈67 ms of work on EPYC, more than the ≈50 ms
  before the next commit.*

> [!NOTE]
> By the work per commit in the table above, `mix20800` at 150K CUs admits 56
> ms, slightly more than the ≈50 ms between two commits, yet its checkpoint lag
> is flat. Work per commit adds up one mean execution time per cost, as if the
> transactions ran one straight after another with nothing else happening; the
> node does not work exactly like that, so the calculation places a
> configuration within about 10 ms rather than exactly. Even so, every run whose
> checkpoint lag climbs over 300 s has more work per commit than every run whose
> checkpoint lag stays flat, and the same calculation predicts which limits keep
> checkpoint lag flat on both machines below.

![Checkpoint lag over the 300 s runs](results/matrix/summary_plots/modes_lag_over_time.png)

*Checkpoint lag over the 300 s runs, Run A (blue) against Run B (orange): the
thin line is the mean per 10 s slice, the steps the mean per 60 s slice, each
taken over the checkpoints of all ten iterations together. Each row is one mix
at two limits. In three of the four panels, checkpoint lag climbs for the whole
five minutes in one of the two runs — Run A for `mix20800` at 100K and 150K, Run
B for `mix10900` at 200K — and that is the run whose commits admit more work, 63
to 73 ms against the ≈50 ms between commits. In the other run, it stays flat
apart from short spikes, and for `mix10900` at 109K it stays flat in both.*

##### Turning the production overshoot on

Every run above sets the overshoot to 0, so a commit admits up to its limit
and no further. Production runs the count limit with an overshoot of 100
(protocol version 22 and later): a commit may admit up to the limit plus the
overshoot, and the excess is carried as debt against later commits. Three
configurations were rerun with the overshoot at 100 on Run A and at
`10 × LIMIT_B` on Run B, ten iterations each, nothing else changed:

| Configuration | Overshoot | Admits/cmt A → B | Success tps A → B | Cancelled/s A → B | Ckpt lag mean s A → B |
| --- | --- | --- | --- | --- | --- |
| `mix20800` at 100K | Off | 10.0 → 29.3 | 193 → 584 | 786 → 409 | 1.66 → 0.59 |
| | On | 10.3 → 28.3 | 199 → 559 | 788 → 425 | 2.19 → 1.06 |
| `mix20800` at 150K | Off | 10.0 → 41.0 | 198 → 821 | 794 → 180 | 1.01 → 0.61 |
| | On | 10.3 → 41.1 | 196 → 822 | 791 → 177 | 1.11 → 0.66 |
| `mix10900` at 109K | Off | 10.0 → 31.5 | 200 → 627 | 791 → 366 | 0.78 → 0.63 |
| | On | 10.3 → 31.4 | 204 → 624 | 784 → 367 | 0.64 → 0.78 |

Turning the overshoot on changes no result by more than it varies between
iterations:

- success tps moves by 2–6 tx/s in Run A, inside its standard deviation of 5–9
  across iterations, and by up to 25 in Run B (`mix20800` at 100K), about its
  standard deviation of 21–23;
- cancellations change by 2–7/s in Run A and by 1–17/s in Run B, each within
  about one standard deviation;
- checkpoint lag stays under 2.2 s in every run and moves by less than one
  standard deviation;
- the success tps ratio between the modes moves from 3.02 to 2.80, 4.14 to 4.19
  and 3.13 to 3.05, less than one step of the ladder.

The overshoot changes nothing because it barely changes what is admitted: Run
A's limit plus overshoot allows 110 transactions per commit, yet Run A admits
10.3 instead of 10. The overshoot is spent in the first commits and then repaid,
so under load, that keeps execution on the object continuous for the whole
window, the admission rate settles back at the limit. The overshoot is there to
absorb a spike, and this workload offers none: it is 1,000 tx/s for 60 s.

##### Doubling the submission rate

The main grid of configurations uses one target rate, 1,000 tx/s, on the
argument that from 5,000 CUs up, the client's in-flight cap already lowers
what it offers to what can be executed on the object (finding 3). That holds
for the heavy fixed-cost configurations. It does not hold for the mixes, where
most transactions are cheap and the drain rate is far higher. Fourteen
configurations were rerun with the target rate at 2,000 tx/s, ten iterations
each, shown as 1,000 tx/s → 2,000 tx/s:

| Configuration | Arrivals/s B | Success tps B | Cancelled/s B | Checkpoint lag mean s B | Success tps B/A |
| --- | --- | --- | --- | --- | --- |
| `cu1k` at 100K | 992 → 1,991 | 994 → 1,988 | 0 → 1 | 0.20 → 0.39 | 4.99 → 9.29 |
| `mix20800` at 150K | 1,000 → 1,948 | 821 → 1,006 | 180 → 942 | 0.61 → 0.76 | 4.14 → 5.14 |
| `mix10900` at 200K | 870 → 1,867 | 693 → 1,058 | 152 → 774 | 1.73 → 1.61 | 3.45 → 5.35 |
| `mix10900` at 109K | 992 → 1,942 | 627 → 733 | 366 → 1,212 | 0.63 → 0.93 | 3.13 → 3.74 |
| `mix3700` at 37K | 999 → 1,934 | 273 → 277 | 727 → 1,662 | 0.34 → 0.84 | 1.38 → 1.41 |
| `cu2k` at 100K | 205 → 339 | 179 → 223 | 0 → 53 | 8.92 → 16.66 | 1.04 → 1.14 |
| `cu2k` at 200K | 205 → 281 | 179 → 224 | 0 → 0 | 8.69 → 17.33 | 1.15 → 1.14 |

Wherever the unit mode was ahead of the count mode at 1,000 tx/s, it stays ahead
at 2,000 tx/s, and within each ladder, the limits that did best still do best.
What changes is how far each configuration was from its own ceiling at 1,000
tx/s:

- Where the limit still had room, doubling the offered load roughly doubles
  success tps. `cu1k` at 100K goes from 994 to 1,988 with cancellations still at
  near zero, so at 1,000 tx/s, that configuration's success tps was set by what
  the client offered, not by the limit.
- Run B's success tps rises by 23 % at `mix20800` at 150K (821 to 1,006) and by
  53 % at `mix10900` at 200K (693 to 1,058), both runs now cancelling heavily,
  which is the first time the limit turns transactions away.
- Where the limit already admits more work than a commit can execute, the extra
  load goes into the backlog instead. `cu2k` at 100K and at 200K roughly double
  their checkpoint lag, 8.9 → 16.7 s and 8.7 → 17.3 s, respectively, for 25 %
  more throughput.

The gap between the modes widens rather than closing — `cu1k` at 100K from 5.0×
to 9.3×, `mix20800` at 150K from 4.1× to 5.1×, `mix10900` at 200K from 3.5× to
5.4× — *so for the configurations where the client was the limit, the gains
measured at 1,000 tx/s are smaller than what a well-chosen unit limit achieves
when more transactions are offered.*

##### Rerunning the `mix20800` ladder on a faster machine

By [What decides the best limit](#what-decides-the-best-limit) above, the work a
commit admits must execute within the ≈50 ms before the next commit, and how
long that work takes depends on the machine. So the four `mix20800`
configurations were rerun on the WS, where a 100K CU transaction executes in 7.4
ms instead of EPYC's 34 ms, 5 iterations each. Run A is the same on both: 202
tx/s, 800 cancelled/s, ≈34 expensive transactions executed per second,
checkpoint lag 0.2–0.9 s on the WS.

| LIMIT_B | Success tps B, WS / EPYC | Cancelled/s B, WS / EPYC | Checkpoint lag mean s B, WS / EPYC | Expensive executed/s B, WS / EPYC |
| --- | --- | --- | --- | --- |
| 100K | 611 / 584 | 391 / 409 | 0.17 / 0.59 | 14 / 14 |
| 150K | 821 / 821 | 181 / 180 | 0.17 / 0.61 | 20 / 20 |
| 208K | 621 / 403 | 381 / 186 | 0.18 / 3.88 | 36 / 33 |
| 500K | 825 / 227 | 176 / 10 | 0.20 / 8.91 | 93 / 42 |

- Up to 150K, the two machines produce roughly the same numbers for everything
  except checkpoint lag, to within a few percent. How many transactions the
  limit admits, and at which cost, is decided by adding up CUs, so it does not
  depend on the machine.
- At 208K, the limit fits two expensive transactions in any commit that has two
  waiting, and leaves 8K CUs for cheap ones. 79 % of Run B's commits admit ≤10
  transactions on the WS, 88 % on EPYC, so success tps drops on both, from 821
  at 150K to 621 on the WS and to 403 on EPYC. On EPYC, checkpoint lag also
  climbs to 3.9 s, because two 100K transactions are 67 ms of execution per 50
  ms commit. On the WS, they are 15 ms, and checkpoint lag stays at 0.18 s.
- At 500K, the two machines produce different numbers altogether. The WS admits
  up to five expensive transactions per commit — 37 ms, inside the 50 ms
  interval — in 63 % of commits, and 100–200 cheap ones in the rest: 825 tx/s,
  matching 150K, at 0.20 s checkpoint lag, while executing **93 expensive
  transactions per second**. That is 4.6× what 150K lets through and 2.8× what
  the count limit does. EPYC, where five would be 170 ms, admits fewer than
  the limit allows (finding 3), executes 42 expensive per second, has a success
  tps of 227, and a checkpoint lag of 8.9 s.

The rule has two parts, and only one of them depends on the machine:

- How many expensive and how many cheap transactions a limit admits does not
  depend on the machine: a limit of exactly two expensive transactions leaves
  room for only eight cheap ones on both machines.
- Whether the expensive transactions it admits execute within the ≈50 ms before
  the next commit does depend on the machine. The WS keeps up at 500K, EPYC does
  not past 150K.

*So a limit of at least one expensive transaction's cost and less than two is
the safe choice on both: the peak throughput on both, no checkpoint lag on
either. On hardware that executes fast enough, larger limits are as good on
success tps and checkpoint lag, and let far more expensive transactions
through.*

##### Rerunning the `mix3700` and `mix50900` ladders on the faster machine

`mix20800` holds the expensive transactions at 100K CUs and varies the limit.
The other two ladders vary that cost itself:

- `mix3700`, where it is 10K CUs and small against the interval;
- `mix50900`, where it is 500K CUs and longer than the whole interval on EPYC.

Both were run on the WS at 5 iterations per configuration, with one limit added
above the top of each EPYC ladder (200K for `mix3700`, 1.5M for `mix50900`):

| Configuration | LIMIT_B | Success tps B, WS / EPYC | Checkpoint lag mean s B, WS / EPYC | Expensive executed/s B, WS / EPYC |
| --- | --- | --- | --- | --- |
| `mix3700` (1K/10K, 7:3) | 10K | 167 / 109 | 0.17 / 1.08 | 4 / 10 |
| | 20K | 211 / 164 | 0.17 / 0.52 | 21 / 26 |
| | 37K | 297 / 273 | 0.18 / 0.34 | 49 / 52 |
| | 100K | 612 / 257 | 0.19 / 7.48 | 154 / 75 |
| | 200K | 1,001 / — | 0.24 / — | 300 / — |
| `mix50900` (1K/500K, 9:1) | 200K | 902 / 901 | 0.16 / 0.32 | 0 / 0 |
| | 509K | 816 / 265 | 0.18 / 8.06 | 19 / 15 |
| | 1M | 918 / 241 | 0.20 / 9.08 | 38 / 20 |
| | 1.5M | 745 / — | 2.05 / — | 55 / — |

**A transaction whose execution takes longer than the ≈50 ms between commits on
one machine can have a good limit on a faster one.** On EPYC, a 500K transaction
takes 81 ms against a 50 ms commit, so every limit that admits one has a
checkpoint lag of 8–9 s, and from EPYC alone, such transactions would seem to
have no good limit at all. On the WS, the same transaction takes 17.7 ms, so two
transactions fit in a commit and three do not. The ladder does exactly that:

- checkpoint lag is 0.16–0.20 s up to 1M, which admits two;
- checkpoint lag is 2.05 s at 1.5M, which admits three;
- 1M gives 918 tx/s against Run A's 202 at the same checkpoint lag, and lets 38
  expensive transactions through per second against Run A's 20.

So whether such transactions have a good limit depends on the machine, not on
the workload.

On the faster machine, `mix3700` improves in the same way and by more. At every
limit, its checkpoint lag stays at 0.17–0.24 s where EPYC's 100K limit has a
checkpoint lag of 7.5 s, and its success tps rises with every step of the
ladder, 167, 211, 297, 612 and 1,001 tx/s, where on EPYC, it peaks at 273 tx/s
at 37K and falls back to 257 tx/s at 100K. At the top limit, success tps equals
the offered load, 1,001 tx/s, with no cancellations at all, and all 300
expensive transactions a second that the mix contains are executed. What holds
it there is how much the client sends, not the limit, so this ladder's peak on
the WS lies above 200K and was not reached at this rate.

The three ladders run on both machines, `mix3700`, `mix20800` and `mix50900`,
show the same two things:

- What a limit has room for is a sum of transactions' CUs, with no machine in
  it.
- Whether the expensive transactions a commit admits fit inside the commit does
  depend on the machine, and that is what decides each ladder's best limit, the
  one with the highest success tps while checkpoint lag stays low:

| Mix | Best limit on EPYC | Best limit on the WS |
| --- | --- | --- |
| `mix3700` (1K/10K) | 37K CUs | above 200K CUs, not reached at this rate |
| `mix20800` (1K/100K) | 150K CUs | 500K CUs |
| `mix50900` (1K/500K) | none: every limit that admits a 500K transaction has 8–9 s of checkpoint lag | 1M CUs |

The first point has one exception. A limit allows the same transactions on both
machines, but the scheduler does not always admit that many. Under a large
backlog of deferred transactions, it admits fewer than the limit allows (finding
3), and fewer on the slower machine: at `mix3700` with a 10K limit, EPYC admits
5.4 transactions per commit and the WS 8.2, although EPYC uses only 11 ms of the
≈50 ms between commits. Where few transactions are deferred, the two machines
admit the same — 41 transactions per commit and 821 tx/s on both at `mix20800`
at 150K — and they differ where more than 700 transactions a second are
cancelled.

![The three mix ladders on both machines](results/matrix/summary_plots/modes_two_machines.png)

*The three mixes run on both machines, EPYC (orange) against the WS (green): Run
B as the line with markers, that machine's Run A dashed, and the shaded band
marks the limits that fit one expensive transaction per commit, not two. The two
machines give nearly the same numbers wherever the limit admits no more work
than EPYC can execute within the ≈50 ms between commits, except at the two
tightest `mix3700` limits, where EPYC admits fewer transactions (the exception
above). At larger limits, EPYC falls behind and the WS does not.*

##### The answer to the H2 question

> H2, as the [stress plan](../stress-plan.md#what-we-want-to-checkconfirm)
> states it: "measure and report the difference in throughput and latency
> between `TotalComputationUnits` and `TotalTxCount`".

**The rule**. *For a shared object whose traffic is mostly cheap, with some
expensive transactions that each take a good part of a commit interval to
execute: set the unit limit at the expensive transaction's cost or above, but
below twice it. One expensive transaction then fits per commit, two never fit,
and the rest of the limit goes to the cheap ones.*

Measured at 1K/100K, at 100K–150K CUs limits, this rule gives the following
advantages:

- 3–4× the count limit's throughput;
- about half its cancellations or fewer, 409 and 180 against ≈790;
- checkpoint lag no higher than the count limit's, and flat over 300 s where
  the count limit's climbs to 7 s;
- unchanged by production's overshoot, and the same peak on both machines.

`10 × mean cost` is the wrong rule. It admits as many expensive transactions as
the mean allows, and on EPYC two of them already overrun the commit.

> [!IMPORTANT]
> The rule is stated in CUs because that is what the limit takes, but what it
> is really about is execution time: a commit should admit about one commit
> interval of work. Everything above follows from that, including which of the
> two runs builds a backlog over 300 s, and which machine keeps up at a given
> limit. A limit set in CUs can only approximate it, and needs the expensive
> transaction's execution time on the machine in question to be set well.

Five things to keep in mind when applying the rule:

- On a machine that fits only one expensive transaction per commit, the unit
  limit's success tps gain over the count limit comes entirely from cheap
  transactions. There the unit limit executes fewer expensive transactions than
  the count limit — 20 per second against 34 at `mix20800` at 150K on EPYC — and
  nearly all of its remaining cancellations are expensive ones that did not fit.
  The count limit counts each transaction as 1 whatever it costs, so it cancels
  cheap and expensive ones alike, about four in five of each.
- The best limit for a given machine is not a fixed number of CUs. How many
  expensive transactions fit in the commit interval depends on their cost and on
  the machine: one 100K transaction on EPYC, five on the WS, where 500K lets
  through 4.6× as many expensive transactions as 150K at the same success tps
  and checkpoint lag; three 10K transactions on EPYC at `mix3700`, where the
  limits that fit only one or two have lower success tps than the count limit.
- A transaction whose execution overruns the commit interval on its own has no
  good limit on that machine, only a choice between checkpoint lag and never
  admitting it. On a machine fast enough to fit it, it behaves like any other:
  at `mix50900` on EPYC, where a 500K transaction takes 81 ms, every limit that
  admits one has 8–9 s of checkpoint lag, while on the WS, where it takes
  17.7 ms, a 1M limit keeps checkpoint lag at 0.20 s with a success tps of 918.
- The unit limit gains success tps only where the count limit's ten transactions
  per commit finish executing well before the next commit, so there is time left
  for more. On EPYC, that needs cheap transactions of about 1K CUs: at
  `mix10900`, making them 2K instead of 1K drops success tps B/A from 3.13× to
  1.05×.
- The count limit is not the safe default it looks like. At `mix20800`, its ten
  admissions hold two 100K transactions on average, 63–65 ms of work per 50 ms
  commit, and its checkpoint lag climbs for a whole 300 s run — at both limits
  its Run B equivalent held flat. Whether a count of 10 is stable depends on
  the mix, and nothing in the count mode lets it be tuned.

---

## H4 — safety (pass/fail)

**PASS.** Across all 2,490 runs of the 124 configurations on EPYC and the 130
runs of the 13 configurations on the WS:

- every safety counter is zero: checkpoint forks
  (`split_brain_checkpoint_forks`, `remote_checkpoint_forks`), inconsistent
  state hash, double-spend attempts, attestation task panics, and soft-lock
  equivocation;
- the per-iteration node state scan found no validator crash, restart, or OOM
  inside any measurement window.

The metric descriptions are in [`../h1/RESULTS.md`](../h1/RESULTS.md), H4 section.
The runs show one unexpected result, noted in finding 1: in two configurations,
Run B's consensus handler could not keep up with the rate consensus produced
commits. It affects performance, not safety.

---

## Summary

The takeaway is the TL;DR at the top of this document. Everything behind it:

- per-configuration numbers: `results/matrix/summary.md` and `summary.csv` for
  EPYC, `results/matrix-ws/summary.md` for the WS — generated by
  [`aggregate.py`](aggregate.py) from the raw runs and not committed;
- figures: `results/matrix/summary_plots/`;
- the cost calibration behind the grid: [`probe-test.md`](probe-test.md).

---

## Potential next steps

- **Rerun the probe and the three mix ladders on mainnet validator hardware.**
  How long the expensive transaction takes to execute decides the best unit
  limit, and it depends on the machine: a 100K transaction takes 34 ms on EPYC
  and 7.4 ms on the WS, so at 1K/100K the best limit is 150K on one and 500K on
  the other. Neither machine is what mainnet validators run. The [hardware
  requirements](../../docs/content/_snippets/operator/validator-requirements-tab.mdx)
  for a mainnet validator are a 24-core processor with 48 vCPUs and 128 GB of
  RAM. They name no CPU model or clock speed, so it is not known whether a
  machine that meets them executes a 100K transaction closer to 34 ms or to
  7.4 ms. Core count does not help here: transactions on one shared object
  execute one after another. The probe on such a machine would give its
  execution time at each cost, and the `mix3700`, `mix20800` and `mix50900` ladders
  would show which limit is best on it. A limit recommended for mainnet should
  come from those runs.
- **Rerun a mix ladder with transactions whose CUs understate their execution
  time.** All workloads here run Move code, where CUs follow execution time.
  Native functions are charged a fixed amount per call instead: a groth16 proof
  check costs about 125 CUs, so one transaction with 700 of them, the most at
  that price, costs about 88K CUs. If it takes much longer than 88K CUs of Move
  code (about 7 ms on the WS), a unit limit lets far more work into a commit
  than its CUs suggest.
