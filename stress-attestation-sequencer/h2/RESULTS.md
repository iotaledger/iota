# H2 — congestion control mode (W5: slow shared-object; count vs units)

**Goal:** measure and report the difference in throughput and latency between
the two per-object congestion control modes.

`TotalTxCount` is the mode every network runs today: it counts each transaction
as 1 against a per-object, per-commit base limit of 10, regardless of the
transaction cost. `TotalComputationUnits` counts each transaction as its
attested computation units (CUs) against a base limit given in the same units.
Each *configuration* — one workload at one limit and rate — is run for ten
iterations, and every iteration runs the same load twice: Run A in
`TotalTxCount`, Run B in `TotalComputationUnits`.

---

## TL;DR

**While every transaction costs the same, the two modes behave identically. They
differ once a commit carries transactions of different cost, and there a well
chosen unit limit does better than the count limit — on throughput,
cancellations, and checkpoint lag at the same time (finding 7).**

- The count limit always admits exactly 10 per commit; the unit limit admits 10
  when a commit holds an expensive transaction and up to `limit / cheap cost`
  when it holds none. At the count limit's equivalent (`10 × mean cost`), that
  already gives 1.3–3.1× the throughput at fewer cancellations.
- But that equivalent is not the best limit, and running each mix at several
  limits shows why. Transactions on the object execute one after another, and a
  new commit arrives every ≈50 ms, so the best limit is the one that puts about
  50 ms of work into each commit: admit less and execution on the object
  finishes early and sits idle, admit more and the leftover piles up. `10 × mean
  cost` is set to admit the same ten transactions the count limit does, so it
  inherits the same flaw: when ten transactions of the mix take more than 50 ms
  to run, both modes admit too much.
- At 1K/100K — cheap transactions of 1,000 CUs mixed with expensive ones of
  100,000 CUs — executing one expensive transaction takes ≈34 ms, so a good unit
  limit fits a single expensive transaction per commit and leaves the rest for
  cheap ones: that puts it between 100,000 and 200,000 CUs.
- A unit limit of 100K–150K CUs gives 3–4× the count limit's throughput, about
  half its cancellations or fewer, and checkpoint lag no higher than the count
  limit's, flat over a 300 s run where the count limit's lag climbs from
  under 1 s to 6–7 s.
- When the expensive transaction is small against the interval (10K CUs, ≈16
  ms), the same rule allows three such transactions per commit, and the limits
  at one to two times the expensive cost admit too little and fall below the
  count limit's throughput.
- One condition bounds the gain: the cheap transactions must be cheap enough
  that a count of 10 leaves execution on the object idle — with transactions
  costing 2K–5K CUs, execution on the object is already saturated and both modes
  behave similarly again.

**The rule — a good unit limit fits one expensive transaction per commit and
gives the rest to cheap ones — was checked three ways, and none of them
changed it (finding 7).**

1. **Overshoot**. Production runs the count mode with an overshoot of 100, not
   the 0 used here. Three configurations were rerun with the overshoot on, and
   nothing changes. Under load that keeps execution on the object continuous,
   the overshoot is spent in the first commits and then repaid, so admissions
   settle back at 10.3 per commit, though the cap allows 110.
2. **Doubled rate**. Doubling the submission rate to 2,000 tx/s changes
  nothing either. But it shows that several configurations were limited by
  the client, not by the limit: at 1K/100K — cheap transactions of 1,000 CUs
  mixed with expensive ones of 100,000 CUs — with a 150K limit, the unit mode
  goes from 821 to 1,006 tx/s, and its improvement over the count mode widens
  from 4.1× to 5.1×.
3. **Five-minute window**. Over five minutes, checkpoint lag separates
  configurations that the 60 s window could not distinguish. The recommended
  limit (150K CUs at `mix20800`) holds flat while the count limit on the same
  mix climbs to 7.3 s. The limit at exactly twice the expensive cost sits at
  1.7 s over 60 s, then climbs to 5.5 s over 300 s and is still climbing at
  the end. In both, the run that climbs is the one admitting more work per
  commit.

**The rule holds on a second machine, but the best limit changes.** The three
limit ladders were rerun on a personal workstation that executes the same
transactions 4.6× faster.

- What a limit has room for does not depend on the machine — it is a sum of
  transactions' CUs. At `mix20800` with a 150K limit, both machines admit 41
  transactions per commit and achieve 821 tx/s.
- Which limit is best does depend on the machine: it is the one whose
  transactions take about 50 ms to execute, and how many transactions fit in
  50 ms changes with how fast the machine runs them.
- Slow execution of transactions within a commit is EPYC's problem, not the
  rule's. At 1K/500K, a single transaction of 500,000 CUs takes ≈81 ms against
  the 50, so no limit works on EPYC. The workstation runs it in 17.7 ms, fits
  two per commit, and has a good limit at 1M: 918 tx/s against the count
  limit's 202 at the same checkpoint lag.
- The unit limit also executes fewer expensive transactions, and that is the
  machine's problem rather than the rule's. At 1K/100K, a 150K limit has room
  for one 100,000 CUs transaction and never two, so it admits exactly 1.00
  expensive transaction per commit against the count limit's 1.73. Where more
  than one expensive transaction fits, it reverses: on the workstation at
  1K/500K with a 1M limit, the unit limit executes 38 expensive transactions
  per second against the count limit's 20 (finding 7).

**Neither limit stops the object from being overloaded because neither
measures time.** From 2,000 CUs per transaction upward, a count limit of 10
admits more than can execute on one shared object: at 2,000 CUs it admits
200 tx/s against a drain rate of 179, and from 5,000 CUs the gap is wide — the
drain rate is 118 tx/s there and 3.8 tx/s at 5,000,000. Both modes admit more
than the drain rate. The transactions that passed the limit wait their turn to
execute on the object. Where the excess ends up depends on the limit:

| While the limit admits | What happens to the excess |
| --- | --- |
| Less than can execute on the object | Cancelled, 850–960/s, checkpoint lag at most 1.5 s |
| More than can execute on the object | Queued: cancellations fall towards zero, checkpoint lag climbs to 4–42 s, settlement latency to 6–29 s at p95 |

Success throughput sits at the drain rate either way, so no setting of either
mode raises it — the limit only picks which failure the users get (findings 3,
4, 5). The cause is structural: a count limit of 10 is ten transactions whether
each takes 0.6 ms or 310 ms, and `10 × cost` in CUs is the same amount of work
regardless of how fast the machine executes a unit. However, a limit in CUs can
at least be set per cost, which a count of 10 cannot.

**When every transaction costs the same, the two modes give the same numbers.**

- 89 of the 137 configurations are fixed-cost: within each one, every
  transaction in a run costs the same. That makes a unit limit of `10 × cost`
  and a count limit of 10 the same limit written two ways, so the two modes
  should produce the same numbers.
- Across the 12 configurations where Run B's limit is exactly ten times the
  transaction's CUs, success throughput matches at B/A = 1.005 (0.984 to 1.025),
  and cancellations, checkpoint lag, and settlement latency match the same way
  (findings 1, 2).
- That is a check on the measurement rather than on the modes. With the two
  identical limits, any gap between the runs would have to come from the
  measurement itself, and there was none — so the gaps in the mixed-cost
  configurations come from the modes. It also means a network switching modes at
  ten times the average CUs loses nothing, and gains nothing, while its traffic
  stays uniform.

> [!NOTE]
> A mode does not always admit as much as its limit allows. Under overload,
> Run A falls from 7.4 to 3.8 transactions per commit against a limit of 10,
> with dozens of deferred transactions and only a quarter of commits scheduling
> anything at the heaviest cost. Both modes do it equally, so it does not
> disturb the comparison, but why the scheduler admits less than its limit is
> not yet known (finding 3).
<!-- -->
> [!IMPORTANT]
> Two things to know before reading the numbers.
>
> - The stress client sends a new transaction only when one of its 2,000
>   in-flight ones completes. At heavy cost, the offered load therefore falls
>   with the network's own latency, from the 1,000 tx/s target at 1,000 CUs to
>   ≈40 tx/s at 5,000,000. "1000 QPS" describes the light configurations only
>   (finding 3).
> - The deferral limit is counted in leader rounds, not scheduling attempts. A
>   skipped leader round uses up a round without a retry, so 1–4 % of deferred
>   transactions — 9–10 % at the heaviest cost — are cancelled after 11–12
>   rounds instead of the configured 10, in both modes alike (finding 6).

**All 2,620 runs passed the safety check (H4):** no checkpoint fork, no
inconsistent state, no double spend, no attestation panic, no soft-lock
equivocation, and no validator crash, restart, or OOM in any of them.

---

## Experiments as run

The configurations form a grid: 12 per-transaction costs, and for each cost,
a ladder of `TotalComputationUnits` limits.

89 of the 137 configurations give every transaction in a run the same cost,
and 48 configurations mix two or three costs in one commit.

- In the 89 fixed-cost configurations, the two modes cannot produce different
  numbers: with one cost, a unit limit of `10 × cost` admits exactly the ten
  transactions a count limit of 10 admits, and any other unit limit has a
  matching count limit. However, these configurations were run to establish
  how a limit behaves on its own — what it admits, where the excess goes, how
  latency follows (findings 1, 3–6) — and, at the matching limit, that the two
  runs give the same numbers, as they must (finding 2).
- In the 48 mixed-cost configurations, the two modes can produce different
  numbers, only when one commit carries transactions of different cost
  (finding 7).

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
  from two or three levels in fixed proportions, written `n:weight`, so one
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
    nothing changed (finding 7).
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
    (finding 7).
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
  - It also writes the spread of each value across the iterations, the rate of
    transactions executed at the expensive level of a mix, the per-commit
    admission histogram (`admits_hist.csv`) and the checkpoint lag per 10 s and
    60 s slice of the window (`lag_over_time.csv`).
- [`plot.py`](plot.py) — renders the figures into `results/matrix/summary_plots/`;
  with `results/matrix-ws` as a second argument, it also draws the data from
  the two machines together.
- Shared with H1: [`../aggregate.py`](../aggregate.py),
  [`../dump_timeseries.py`](../dump_timeseries.py) and [`../exp_dir.py`](../exp_dir.py).

> [!NOTE]
> The stress plan names `transactions_included_in_checkpoint` as H2's
> throughput metric. It is reported here as the finalized rate, but the
> headline throughput is **success tps = executed − cancelled − commit
> prologues**: user transactions that did real work. Checkpoint inclusion lags
> execution by the checkpoint lag, and once that lag approaches the 60 s
> window, it undercounts what the window processed — at the three heaviest cost
> points, `included − cancelled − commit prologues` comes out at 0.0, −13.4 and
> −27.8 tx/s. Cancelled transactions are subtracted because they execute but do
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

Keep the client's in-flight cap and the 60 s window in mind when reading the
heavy-cost numbers:

- **The offered load is not 1,000 tx/s.** The client keeps at most 2,000
  transactions in flight and waits for each to finish, so it can offer only
  2,000 divided by the current latency. At 1,000 CUs, that is the full target;
  at 5,000 CUs, about 200 tx/s arrive; at 500,000 CUs, about 80 tx/s; at
  5,000,000 CUs, about 40 tx/s (finding 3).
- **Checkpoint lag is measured only for checkpoints built inside the window.** A
  backlog that outlives the 60 s run is never recorded, so the worst-lagging
  checkpoints are missing from the average and the lag mean becomes lower than
  the truth. The bigger the backlog, the further the mean falls below the truth.
  The share of checkpoints past 30 s is the reliable measure of the tail. A
  quantile beyond 30 s is not, because the histogram's buckets there are 30, 60,
  90 and then unbounded, so the value is interpolated across a 30-second gap
  rather than observed — the tables print `>30` instead.

In the figures, one curve corresponds to one cost point, coloured from light to
dark by cost; Run A is drawn as a star or a vertical line, since its admitted
rate is the same (to a few percent) in every configuration of a point.

---

**1. Each limit admits exactly the number it specifies: 10 per commit for the
count limit, `LIMIT_B / cost` rounded down for the unit limit.**

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
  admits less — 7.2 per commit, then 4.1, then 1.2 — although dozens sit
  deferred each commit. Admitting less than the limit allows is finding 3,
  and its cause is open.
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
0, so in the 8 configurations whose limit is under one transaction's cost, Run
B cancels everything that arrives after ten rounds — 740–950 per second in six
of the eight, fewer in the two heaviest for the reason below — and completes
0.4–1.7 tx/s. Run A in the same configuration is unaffected. Once the limit
allows more than can execute on the object per commit, it stops deciding
anything and Run B's admits track Run A's instead (finding 3).

> [!NOTE]
> Two of those eight configurations show something else. In `cu2m-lim1m` and
> `cu5m-lim2m`, Run B's consensus handler could not keep up with the rate
> consensus produced commits, processing only 11.6 and 7.2 commits per second
> against Run A's 19.7 and 18.5, with skipped leader rounds climbing to 74–77
> per run from Run A's 24–45 (finding 6). Every transaction in those runs is
> deferred and re-evaluated every commit at 2–5 million CUs each. It is the
> only place in the grid where the mode changed the commit rate. Why the
> handler could not keep up is left for a follow-up.

---

**2. At the matched limit, the two modes produce the same numbers, at every cost
from 1,000 to 5,000,000 CUs.**

The 12 configurations where `LIMIT_B = 10 × cost` are the unit limit equivalent
of Run A's count limit of 10. In every one of them, the two runs admit the same
number of transactions per commit and achieve the same throughput:

| Cost point | Admits/cmt A → B | Success tps A → B | B/A | Cancelled/s A → B | Ckpt lag mean s A → B |
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

- Success throughput B/A averages 1.005 over the twelve, between 0.984 and
  1.025 — closer to 1 than the spread between iterations of a single run, so
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
Run A and every Run B limit at each cost point](results/matrix/summary_plots/modes_heatmaps.png)

*Every configuration at a glance: success tps, cancelled fraction of offered,
checkpoint lag mean, and the share of checkpoint lag over 30 s, one panel each.
Run A is the left column and each `LIMIT_B` follows to its right, with the cost
points down the side. Within a panel, the same colour means the same value;
success tps and checkpoint lag use a log colour scale. The outlined cells are
the `10 × cost` limit, the one that matches Run A: in the success panel they
read 193 against Run A's 195 at `cu1k`, 176 against 172 at `cu2k`, and so on
down the column.*

---

**3. Throughput is set by how fast transactions on one object execute, one
after another. From 2,000 CUs up, each limit admits more transactions per
second than can be executed on the object.**

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `execution_driver_executed_transactions` | Cumulative number of transactions executed by execution driver | Counter; rate, averaged across validators and iterations. Counts cancelled transactions and the per-commit system transaction too, so success tps subtracts both |
| `transactions_included_in_checkpoint` | Transactions included in a checkpoint | Counter; rate, averaged across validators — the finalized rate, prologues included |
| `consensus_handler_deferred_transactions` | Number of transactions deferred by consensus handler | Counter; rate. A transaction counts once for every commit it stays deferred |

</details>

Transactions on one mutable shared object execute one after another, so at each
cost, there is a top rate at which they complete — the drain rate in the table.
It is measured in the configurations whose limit lets in more than can be
executed on the object: there the limit is not what holds throughput down, so
success settles at that rate and does not rise as the limit goes higher. The
rate at 5,000,000 CUs is nearly 50 times lower than at 2,000, while the count
limit stays at 10 per commit throughout:

| Cost point | CUs per tx | Drain rate (tx/s) | Arrivals/s | Scheduled/s | Success tps | Scheduled / success |
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
mean over the four validators. The drain rate is the median of the success
rates described above. Scheduled is how many transactions per second the
scheduler let onto the object, the admits histogram's `_sum` over time;
arrivals is scheduled plus cancelled, everything the client got into
consensus.

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
success by 13 % at 2,000 CUs and by 7× at 5,000,000 — every scheduled
transaction beyond the drain rate joins an execution backlog that grows for
the whole window. Both limits let this happen because neither is a measure
of time: a count of 10 is the same ten transactions whether each takes 0.6
ms or 310 ms, and `10 × cost` units is the same amount of "work" whether the
machine executes a unit in a nanosecond or a microsecond. The backlog is
what checkpoint lag measures (finding 4).

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

**4. The limit chooses where the excess lands: cancelled by the
scheduler, or queued for execution.**

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `checkpoint_creation_latency` | Latency from consensus commit timestamp to local checkpoint creation in milliseconds | Histogram; the exact mean from `Δ_sum / Δ_count` and the share of observations above the 30 s bucket edge, combined across validators over all iterations. Quantiles past 30 s print as `>30` |

</details>

At every cost point, raising the limit runs through the same two stages:

- While the limit admits fewer transactions than can be executed on the object,
  the excess is deferred and cancelled at ten rounds: 830–960/s, most of what
  arrives. Checkpoint lag stays at 1.5 s or below, because nothing scheduled is
  waiting to execute.
- At the first limit whose admitted rate reaches the drain rate, the picture
  turns around. Cancellations fall towards zero, success tps flattens at the
  drain rate, and checkpoint lag jumps by five to forty times and goes on
  rising as the limit loosens:

| Cost point | Drain rate (tx/s) | Last limit before the turn: admitted/s → checkpoint lag s, cancelled/s | First limit after the turn: admitted/s → checkpoint lag s, cancelled/s |
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

The three heaviest points are over the drain rate at their very first limit:
one transaction per commit is already 18–20 tx/s against a drain of 4–15. For
them every admitting limit builds a backlog.

Past that point, raising the limit further adds no throughput:

- at `cu50k`, the limits admitting 4 to 100 transactions per commit all complete
  60–64 tx/s, while the lag mean climbs from 6 s to 27 s and the share of
  checkpoints past 30 s from 0 to 30 %;
- at `cu100k` and `cu200k`, the top limits put 85–90 % of checkpoints past 30 s,
  and their lag means of 33–42 s are lower bounds (see the note on the 60 s
  window above).

> [!IMPORTANT]
> This is the actual trade the limit makes. A tight limit turns the excess
> into cancellations, which the client sees as failures within ten rounds and
> can resubmit. A loose one turns it into a queue, which the client sees as
> latency and the network as checkpoint lag. Success throughput is the same
> either way once execution on the object is saturated, so no setting of either
> mode raises it. The only choice is which failure the users get.

![Checkpoint lag mean against success tps per configuration](results/matrix/summary_plots/modes_tradeoff.png)

*Checkpoint lag mean against success tps, one point per configuration, Run A
starred. Lower-right is fast and stable; the configurations arc up and right as
the limit loosens: no extra throughput, more lag.*

![The same curves over the allowed rate divided by the drain rate](results/matrix/summary_plots/modes_utilization.png)

*The curves of finding 3 with the x-axis divided by each cost point's drain
rate, lag above and cancelled fraction below. The cancelled curves fall onto
almost one line, the two outcomes trading places just past the dashed line,
where a limit allows exactly the drain rate. The lag curves rise at the same
place but settle at different heights, 9 s at `cu2k` to 41 s at `cu200k`.*

---

**5. Latency: identical between modes at matched limits; set by the
backlog, not the mode.**

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `transaction_driver_settlement_finality_latency` | Settlement finality latency observed from transaction driver | Histogram on the fullnode (the runs submit through it); p50/p95 over buckets combined across iterations |
| `validator_transaction_execution_latency` | Validator-internal latency from receiving a transaction via `submit_tx` until it finished executing (pre-consensus check, consensus, post-consensus validation, sequencing incl. deferral, execution) | Histogram; p95 over buckets combined across validators and iterations |
| `authority_state_internal_execution_latency` | Latency of actual certificate executions | Histogram; mean, validators only. Blends the per-commit system transactions and the cancelled transactions in with the workload, so it understates the workload's cost where cancellations are many |

</details>

At the twelve matched configurations the client-facing latency is the same in
both modes at every cost point (each run's own values, as in finding 2):

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

The p95 values match within 1–5 %. The medians move by up to 11 % between
the runs (`cu10k`, `cu100k`), which is inside the spread of Run A's own
median across the configurations of a cost point (7.8 to 8.9 s at `cu10k`).

Along a ladder, latency follows the backlog of finding 4 and nothing else:

- Every limit that admits less than the drain rate settles in ≈ 730–750 ms at
  the median and ≈ 900 ms at p95, at every cost from 1,000 to 500,000 CUs. A
  transaction that is admitted executes at once, and the rest are cancelled
  within ten rounds.
- The first limit over the drain rate takes p95 to 6–25 s while the median
  mostly holds near 750 ms, because the queue is not yet long enough to reach
  the typical transaction. One or two limits further, the median follows, to
  3–16 s.
- Receipt-to-executed p95, the validator's own view including time spent
  deferred, reaches 45–56 s at the heavy points, close to the window length.
- The median does not fall steadily down the cost column — 8.6 s at `cu20k`,
  0.86 s at `cu500k` — because the client's offered load shrinks with cost.
  With fewer transactions in flight, the ones that do get admitted wait behind
  fewer others.

The VM execution column is the blended
`authority_state_internal_execution_latency` (the user-only variant was added
after these runs). It grows from 0.23 to 25 ms across the cost range on this
machine, and is identical between the modes: cost is a property of the
transaction, and the mode only decides how many of them are let in.

---

**6. The deferral limit is counted in leader rounds, so a skipped round
cancels transactions a round early — in both modes.**

<details>
<summary>Metric descriptions</summary>

| Metric | Codebase description | Aggregation |
| --- | --- | --- |
| `consensus_handler_transaction_deferral_rounds` | Number of consensus rounds a transaction spent deferred before it was scheduled or cancelled | Histogram; share of observations above 10, combined across validators and iterations |
| `consensus_handler_leader_round` | The leader round of the current consensus output being processed in the consensus handler | Gauge; its advance over a run minus the commits in that run = skipped leader rounds (validator-1) |

</details>

With `max_deferral_rounds = 10` a deferred transaction should be cancelled
on its tenth round, so the deferral-rounds histogram should never exceed 10.
It does, in every configuration of both modes: 1 % of deferral resolutions at
`cu1k` under Run A land in the (10, 20] bucket, 4 % under Run B, 1–3 % at
the heavy points — 496,000 observations in Run A and 1,594,000 in Run B
over the whole grid. The excess is small (the real values are 11–12) and it
is not a difference between the modes.

The mechanism is how the rounds are counted.

- A transaction's deferral key records the leader round it was first deferred
  from (`DeferralKey::ConsensusRound { future_round, deferred_from_round }`).
  The limit check compares `future_round − deferred_from_round` against the
  maximum, and the round it uses is the commit's leader round
  ([`consensus_handler.rs:243`](../../crates/iota-core/src/consensus_handler.rs#L243),
  `consensus_output.leader_round()`).
- Consensus does not produce a commit for every leader round. Over a 60 s run
  the leader round advances more than the number of commits by 7 to 77, in most
  configurations by 20 to 40, in both modes and at every cost point. 20 to 40 is
  2–3 % of ≈ 1,200 commits; the two configurations of finding 1 whose handler
  fell behind are the 74 and 77. The two 300 s configurations skip 111 to 176,
  the same share of a window five times longer.
- Each skipped round moves the counter without a scheduling attempt. So a
  transaction deferred across one skipped round is cancelled after nine
  evaluations, and the rounds it is charged
  (`commit_round.saturating_sub(deferred_from_round)`,
  [`authority_per_epoch_store.rs`](../../crates/iota-core/src/authority/authority_per_epoch_store.rs)) read 11 or 12.

> [!IMPORTANT]
> The effect on the numbers here is small, but the limit is not what its name
> says. It is ten leader rounds, not ten chances to be scheduled, and under
> consensus conditions that skip more rounds the gap grows. Counting
> evaluations rather than the round difference would make the two the same.
> Worth an upstream issue.

---

**7. When one commit carries transactions of different cost the modes differ,
and a well-chosen unit limit does better than the count limit on throughput,
cancellations, and lag at the same time.**

The six findings above hold cost fixed within a run, so the two modes could only
produce the same numbers. The `SLOW_MIX` configurations give the transactions in
one commit two or three costs. Two checks say they measure what they were meant
to:

- The mix is what was configured, in every one of them: the expensive share
  within half a percentage point of design, the mean cost within 3 %.
- Run A's count limit binds wherever execution on the object still has spare
  time (`admits/cmt` 10.00).

*At the count limit's equivalent.* The first five mixes ran at
`LIMIT_B = 10 × mean cost`, so Run B's limit is the work Run A admits on
average and the spread is the only difference:

| Configuration | Costs (units), ratio | Admits/cmt A → B | Success tps A → B | B/A | Cancelled/s A → B | Ckpt lag mean s A → B |
| --- | --- | --- | --- | --- | --- | --- |
| `mix1900` | 1K / 10K, 9:1 | 10.00 → 13.3 | 200.9 → 265.9 | 1.32 | 793 → 719 | 0.42 → 0.44 |
| `mix3700` | 1K / 10K, 7:3 | 10.00 → 13.6 | 198.4 → 272.9 | 1.38 | 788 → 727 | 0.55 → 0.34 |
| `mix10900` | 1K / 100K, 9:1 | 10.00 → 31.5 | 199.9 → 626.7 | 3.13 | 791 → 366 | 0.78 → 0.63 |
| `mix20800` | 1K / 100K, 4:1 | 10.00 → 21.6 | 194.7 → 402.9 | 2.07 | 786 → 186 | 0.74 → 3.88 |
| `mix50900` | 1K / 500K, 9:1 | 10.0 → 14.8 | 181.9 → 265.0 | 1.46 | 788 → 31 | 2.34 → 8.06 |

Run A admits exactly 10 in every commit — 100 % of its commits sit in the
7–10 bucket of the admission histogram. Run B's limit is in CUs, so what
it admits depends on what the commit holds. At `mix1900` the limit is
19,000 CUs: a commit holding one 10,000 CU transaction has room for 9
cheap ones (10 in all), a commit holding none fits 19 — measured, 63 % of
commits at 7–10 and 37 % at 10–20. At `mix10900` the limit is 109,000: one
expensive plus 9 cheap, or 109 cheap — 78 % of commits at 7–10 and 22 % at
100–200. The extra admissions are cheap transactions that a count of 10 would
have deferred and, mostly, cancelled after ten rounds; cancellations fall
from ≈790/s in every Run A to 719, 727, 366, 186 and 31/s.

*Which limit.* Four of those mixes were then run at several limits. This is
the question the stress plan asks, in the only setting where the answer can
depend on the mode:

| Configuration | LIMIT_B | Admits/cmt B | Success tps B | Cancelled/s B | Lag mean s B | Expensive executed/s B | Run A: success, cancelled, lag, expensive executed/s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `mix3700` (1K/10K, 7:3) | 10K | 5.4 | 109 | 872 | 1.08 | 10 | 199, 792, 0.4–1.4, 59 |
| | 20K | 8.1 | 164 | 836 | 0.52 | 26 | |
| | **37K** | 13.6 | **273** | 727 | **0.34** | 52 | |
| | 100K | 16.2 | 257 | 28 | 7.48 | 75 | |
| `mix10900` (1K/100K, 9:1) | 50K | 45.3 | 902 | 100 | 0.53 | 0 | 200, 790, 0.3–0.8, 17 |
| | 109K | 31.5 | 627 | 366 | 0.63 | 16 | |
| | **200K** | 36.0 | **693** | 152 | 1.73 | 32 | |
| | 500K | 22.6 | 375 | 0 | 5.79 | 37 | |
| `mix20800` (1K/100K, 4:1) | 100K | 29.3 | 584 | 409 | **0.59** | 14 | 195, 791, 0.7–1.7, 34 |
| | **150K** | 41.0 | **821** | 180 | **0.61** | 20 | |
| | 208K | 21.7 | 403 | 186 | 3.88 | 33 | |
| | 500K | 13.8 | 227 | 10 | 8.91 | 42 | |
| `mix50900` (1K/500K, 9:1) | 200K | 46.2 | 901 | 100 | 0.32 | 0 | 182, 791, 2.3–2.4, 16 |
| | 509K | 14.8 | 265 | 31 | 8.06 | 15 | |
| | 1M | 14.4 | 241 | 5 | 9.08 | 20 | |

**The count limit's equivalent is not the best limit.** At `mix20800`:

- 208K CUs (`10 × mean cost`) admits two 100K transactions per commit and 8
  cheap ones. 88 % of Run B's commits sit at ≤10, and lag is 3.9 s.
- One step down at 150K, only one expensive transaction fits and the remaining
  50K CUs go to cheap ones. Run B admits 41 per commit (50 % of commits at
  20–50, 41 % at 50–70), completes **821 tx/s against Run A's 198**, cancels
  180/s against 795, and its checkpoint lag is **0.61 s against Run A's 1.0 s**.
- At 100K exactly one expensive transaction fits and nothing else, so commits
  alternate between one expensive transaction (71 %) and 70–100 cheap ones
  (29 %): 584 tx/s, lag 0.59 s.

The other two ladders have the same shape:

- `mix10900`: 200K does better than 109K on throughput (693 against 627) and
  cancellations (152 against 366). But 200K is exactly twice the expensive
  cost, so two fit per commit, and lag rises to 1.7 s against Run A's
  0.3–0.8 s. 500K falls to 375 tx/s at 5.8 s.
- `mix3700`, where the expensive transaction is 10K: here the `10 × mean cost`
  limit is the best, at 37K, three expensive plus seven cheap. 10K and 20K fit
  only one or two expensive transactions and fall below the count limit, 109 and
  164 tx/s against 199. 100K fits ten expensive and lags 7.5 s.

**Why: what the limit admits must fit the commit interval in execution time.**
Transactions on the object execute one after another, and consensus commits
every ≈50 ms. On this machine a 100K CU transaction takes ≈34 ms, a 10K one ≈16
ms, a 500K one ≈81 ms, and a cheap one 0.6 ms ([`probe-test.md`](probe-test.md)). Working each
case through against the 50 ms a commit has:

| What a commit admits | Execution time | Result |
| --- | --- | --- |
| One 100K plus fifty cheap | ≈62 ms | Execution on the object keeps up, lag under a second |
| Two 100K, before any cheap | 67 ms | Lag climbs |
| Three 10K plus seven cheap (`mix3700` at 37K) | 52 ms | Fine |
| Ten 10K (`mix3700` at 100K) | 159 ms | 7.5 s of lag |
| One 500K | 81 ms | Over the interval on its own |

The last row is why `mix50900` lags at *every* limit that admits an expensive
transaction on this machine: 8.1 s at 509K, 9.1 s at 1M. No unit limit fixes a
level whose single transaction overruns the commit, though a faster machine can
make the same level fit (the WS ladders below).

The count limit has the same problem and no knob at all. At `mix20800` its ten
admissions hold two expensive transactions on average, so Run A's lag grows
too, for the whole of a 300 s run (below).

> [!WARNING]
> **A limit below the expensive cost meets the goal by dropping a whole
> level.** At `mix10900`/50K and `mix50900`/200K, Run B never admits an
> expensive transaction. It cancels exactly that level — 100/s, the 10 % that
> arrive — and runs the cheap ones at 900 tx/s with 0.3–0.5 s lag. Fewest
> cancellations and no lag, because one level of transaction is never let
> through. Any rule for choosing the limit has to exclude this: every level
> still has to get through.

![Success, cancellations and lag against the limit, for the four mixes run at several limits](results/matrix/summary_plots/modes_mix_ladders.png)

*Run B (orange) against `LIMIT_B`, with one standard deviation across
iterations as error bars; Run A (blue dashes, band = its spread) as the
reference. The dotted line marks the expensive transaction's cost, the
dash-dotted line `10 × mean cost`, and the shaded band runs from the expensive
cost to twice it. The bottom row is the expensive level's execution rate.
At 1K/100K success peaks inside the band and lag rises at the limit that fits
two expensive transactions; at 1K/10K the peak is at 37K, three per
commit.*

*What the gain needs.* Raising the cheap level kills the gain. Holding the
expensive level fixed and making the cheap one costlier:

| Mixes | Cheap level | Gain |
| --- | --- | --- |
| `mix50900` → `mix54500` → `mix68000` | 1K → 5K → 20K | 1.46× → 1.10× → 1.07× |
| `mix10900` → `mix11800` | 1K → 2K | 3.13× → 1.05× |
| `mix9500` | 5K | 1.04× |

In those configurations both runs' admission histograms sit below 10 and Run A's
lag is already 9–23 s. Execution on the object is kept saturated by the "cheap"
level alone — 5K transactions drain at 118/s, 2K at 179/s (finding 3) — so there
is no idle capacity for the unit limit to fill.

So the gain is not set by how far apart the two costs are. It is set by whether
a count of 10 leaves execution on the object idle, which needs the cheap level
to be cheap against the drain rate on the object, ≈1,000 CUs here.

Raising the expensive share does the same from the other side, because past
20 % the expensive level alone fills the object:

| Mix | Expensive share | Gain | Lag A → B |
| --- | --- | --- | --- |
| `mix10900` | 10 % | 3.1× | 0.8 → 0.6 s |
| `mix20800` | 20 % | 2.1× | 0.7 → 3.9 s |
| `mix30700` | 30 % | 1.4× | 7.3 → 9.7 s |
| `mix50500` | 50 % | 1.3× | 15.3 → 15.6 s |

The three-level mix (`mix13600`, 1K/10K/100K at 60/30/10 %) gains 1.17× with
lag 5.5 → 9.7 s: its 10K middle level uses the capacity the cheap ones would
have filled.

![Admitted per commit, and the outcome, for the mixes at the count limit's equivalent](results/matrix/summary_plots/modes_mix.png)

*The twelve mixes run at `LIMIT_B = 10 × mean`. Top: the share of commits
admitting each number of transactions, Run A (blue) next to Run B (orange).
Bottom: success, cancellations and lag. Run B gains where Run A's bar is 100 %
at 7–10 — where the count limit binds and execution on the object has spare
time; where both runs have slid below 10 execution on the object is already
saturated and the modes produce the same numbers.*

*Stable, not just low.* The 60 s window cannot tell a queue that is high but
stable from one that keeps growing, so four configurations ran for 300 s, ten
iterations each: the two recommended limits, the count limit's equivalent at
`mix20800`, and the limit at exactly twice the expensive cost at `mix10900`.
Checkpoint lag per 60 s slice of the window, the exact mean over the
checkpoints built in that slice across the iterations ([`lag_over_time.csv`](results/matrix/lag_over_time.csv)):

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

Throughput over 300 s matches the 60 s runs to within 2 % everywhere. Lag
does not. In three of the four configurations one of the two runs climbs for the
whole five minutes without levelling off, and which run climbs follows the
same arithmetic as the rest of this finding — the work a commit admits
against the ≈50 ms interval, at ≈34 ms per 100K transaction and 0.6 ms per
cheap one:

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

Two things follow.

- The recommended limit holds over five minutes. At `mix20800`/150K Run B's lag
  is 0.3–0.6 s in every one of the five minutes, while Run A, the count limit,
  climbs to 7.3 s. That is the same limit that already gave 4.1× the throughput
  and under a quarter of the cancellations. So at this mix the unit limit is
  not only faster and cancelling less, it is the run that stays stable, and the
  count limit is the one that does not.
- The limit at exactly twice the expensive cost is worse than 60 s made it look.
  `mix10900` at 200K reads 1.7 s over 60 s, mild enough to look like a working
  limit. Over 300 s it climbs to 5.5 s and is still climbing at the end. Its
  throughput advantage over 109K, 693 against 627, is a backlog being built and
  not capacity being used. That settles the question left open in the ladder
  table above: 200K is not a better limit than 109K, it is an unstable one, and
  the upper bound of the rule holds.

> [!NOTE]
> One row needs care. `mix20800` at 150K admits 56 ms of work by this
> arithmetic, slightly more than the interval holds, yet its lag is flat. The
> estimate uses one mean execution time per level and ignores whatever the node
> overlaps, so it places a configuration within about 10 ms rather than exactly.
> It sorts these four configurations correctly and it predicts both machines
> below, which is what it is used for here.

![Checkpoint lag over the 300 s runs](results/matrix/summary_plots/modes_lag_over_time.png)

*Checkpoint lag over the 300 s runs, Run A (blue) against Run B (orange):
the thin line is the mean per 10 s slice, the steps the mean per 60 s
slice, all iterations pooled. The two configurations on the left share a mix and
differ only in the limit, as do the two on the right. In each pair the run
whose commits admit more than ≈50 ms of work climbs for the whole five
minutes, and the other holds apart from short spikes.*

*With production's overshoot on.* Every run above sets the overshoot to 0, so a
commit admits up to its limit and no further. Production runs the count limit
with an overshoot of 100 (protocol version 22 and later): a commit may admit up
to the limit plus the overshoot, and the excess is carried as debt against later
commits. Three configurations were rerun with the overshoot at 100 on Run A and
at `10 × LIMIT_B` on Run B, ten iterations each, nothing else changed:

| Configuration | Overshoot | Success tps A | Cancelled/s A | Admits/cmt A | Success tps B | B/A |
| --- | --- | --- | --- | --- | --- | --- |
| `mix20800` at 100K | Off | 193.2 | 786 | 10.00 | 584 | 3.02 |
| | On | 199.5 | 788 | 10.3 | 559 | 2.80 |
| `mix20800` at 150K | Off | 198.4 | 795 | 10.00 | 821 | 4.14 |
| | On | 196.1 | 791 | 10.3 | 822 | 4.19 |
| `mix10900` at 109K | Off | 199.9 | 791 | 10.00 | 627 | 3.13 |
| | On | 204.4 | 784 | 10.3 | 624 | 3.05 |

Nothing moves:

- success throughput changes by less than its own spread across iterations,
  4.9–8.5 tx/s;
- cancellations change by 3–7/s, against a spread of 10–18;
- the ratio between the modes changes by less than one step of the ladder.

The admissions say why. Run A's limit plus overshoot allows 110 per commit, and
Run A admits 10.3. The overshoot is spent in the first commits and then repaid,
so under load that keeps execution on the object continuous for the whole
window, the admission rate settles back at the limit. The overshoot is there to
absorb a spike, and this workload offers none: it is 1,000 tx/s for 60 s.

> [!IMPORTANT]
> So the comparison in this finding, run at overshoot 0, is the comparison
> production would see for traffic that saturates the object. It says nothing
> about bursty arrivals, which is the case the overshoot exists for.

*At twice the submission rate.* The grid uses one target rate, 1,000 tx/s, on
the argument that from 5,000 CUs up the client's in-flight cap already lowers
what it offers to what can execute on the object (finding 3). That holds for the
heavy fixed-cost configurations. It does not hold for the mixes, where most
transactions are cheap and the drain rate is far higher. Fourteen configurations
were rerun with the target at 2,000 tx/s, ten iterations each, shown as 1,000
tx/s → 2,000 tx/s:

| Configuration | Offered tx/s B | Success tps B | Cancelled/s B | Lag mean s B | B/A |
| --- | --- | --- | --- | --- | --- |
| `cu1k` at 100K | 994 → 1,989 | 994 → 1,988 | 0 → 1 | 0.20 → 0.39 | 4.99 → 9.29 |
| `mix20800` at 150K | 1,001 → 1,948 | 821 → 1,006 | 180 → 942 | 0.61 → 0.76 | 4.14 → 5.14 |
| `mix10900` at 200K | 845 → 1,832 | 693 → 1,058 | 152 → 774 | 1.73 → 1.61 | 3.45 → 5.35 |
| `mix10900` at 109K | 993 → 1,945 | 627 → 733 | 366 → 1,212 | 0.63 → 0.93 | 3.13 → 3.74 |
| `mix3700` at 37K | 1,000 → 1,939 | 273 → 277 | 727 → 1,662 | 0.34 → 0.84 | 1.38 → 1.41 |
| `cu2k` at 100K | 179 → 276 | 179 → 223 | 0 → 53 | 8.92 → 16.66 | 1.04 → 1.14 |
| `cu2k` at 200K | 179 → 224 | 179 → 224 | 0 → 0 | 8.69 → 17.33 | 1.15 → 1.14 |

Nothing reverses. Every configuration keeps its place and the ladder keeps its
shape. What changes is how far each configuration was from its own ceiling at
1,000 tx/s:

- Where the limit still had room, doubling the offered load roughly doubles
  what completes. `cu1k` at 100K goes from 994 to 1,988 tx/s with cancellations
  still at zero, so at 1,000 tx/s that configuration was measuring the client
  and not the limit.
- `mix20800` at 150K gains 23 % and `mix10900` at 200K gains 53 %, both
  starting to cancel heavily, which is the limit binding for the first time.
- Where the limit already admits more work than a commit can execute, the extra
  load goes into the backlog instead. `cu2k` at 100K and at 200K roughly double
  their lag, 8.9 → 16.7 s and 8.7 → 17.3 s, for 25 % more throughput.

The gap between the modes widens rather than closing — `cu1k` at 100K from 5.0×
to 9.3×, `mix20800` at 150K from 4.1× to 5.1×, `mix10900` at 200K from 3.5× to
5.4× — so the 1,000 tx/s numbers in this finding understate what a well-chosen
unit limit does.

> [!NOTE]
> Two rows are not a like-for-like rate comparison. `cu2k`'s 1,000 tx/s runs
> delivered only 397 and 179 tx/s against the target, so that pair differs in
> what arrived as well as in what was asked for. The `mix` rows and `cu1k`
> delivered 98–100 % of both targets.

*The same ladder on the WS.* The explanation above says the limit is really
about execution time per commit, which is a property of the machine. So the
four `mix20800` configurations were rerun on the WS, where a 100K CU
transaction executes in 7.4 ms instead of EPYC's 34 (5 iterations each). Run A
is the same on both: 202 tx/s, 800 cancelled/s, ≈34 expensive transactions
executed per second, lag 0.2–0.9 s on the WS.

| LIMIT_B | Success tps B, WS / EPYC | Cancelled/s B, WS / EPYC | Lag mean s B, WS / EPYC | Expensive executed/s B, WS / EPYC |
| --- | --- | --- | --- | --- |
| 100K | 611 / 584 | 391 / 409 | 0.17 / 0.59 | 14 / 14 |
| 150K | 821 / 821 | 181 / 180 | 0.17 / 0.61 | 20 / 20 |
| 208K | 621 / 403 | 381 / 186 | 0.18 / 3.88 | 36 / 33 |
| 500K | 825 / 227 | 176 / 10 | 0.20 / 8.91 | 93 / 42 |

- Up to 150K the two machines produce the same numbers for everything but lag,
  to within a few percent. How many transactions the limit admits, and of which
  level, is arithmetic on CUs.
- At 208K the limit fits two expensive transactions in any commit that has two
  waiting, and leaves 8K CUs for cheap ones. 79 % of Run B's commits admit ≤10
  on the WS, 88 % on EPYC, so throughput dips on both. On EPYC the dip is
  deeper, 403 against 621, and lag climbs to 3.9 s, because two 100K
  transactions are 67 ms of execution per 50 ms commit. On the WS they are
  15 ms, and lag stays at 0.18 s.
- At 500K the two machines produce different numbers altogether. The WS admits
  up to five expensive transactions per commit — 37 ms, inside the interval — in
  63 % of commits, and 100–200 cheap ones in the rest: 825 tx/s, matching 150K,
  at 0.20 s lag, while executing **93 expensive transactions per second**. That
  is 4.6× what 150K lets through and 2.8× what the count limit does. EPYC, where
  five would be 170 ms, admits fewer than the limit allows (finding 3), executes
  42 expensive per second, completes 227 tx/s, and lags 8.9 s.

The rule has two parts, and only one of them depends on the machine:

- How the limit splits between the two levels does not. A limit of exactly two
  expensive transactions crowds the cheap ones out on both machines.
- Whether the expensive transactions it admits fit the commit does. The WS
  keeps up at 500K, EPYC does not past 150K.

So a limit of at least one expensive transaction's cost and less than two is
the safe choice on both: the peak throughput on both, no lag on either. On
hardware that executes fast enough, larger limits are as good on throughput and
lag, and let far more of the expensive level through.

*Two more ladders on the WS.* `mix20800` holds the expensive level at 100K CUs
and varies the limit. The other two ladders vary the expensive level itself:

- `mix3700`, where it is 10K CUs and small against the interval;
- `mix50900`, where it is 500K CUs and longer than the whole interval on EPYC.

Both were run on the WS at 5 iterations per configuration, with one limit added
above the top of the EPYC ladder:

| Configuration | LIMIT_B | Success tps B, WS / EPYC | Lag mean s B, WS / EPYC | Expensive executed/s B, WS / EPYC |
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

**A level whose transaction overruns the commit has a good limit on a machine
where it does not overrun it.** On EPYC a 500K transaction takes 81 ms against
a 50 ms commit, so every limit that admits one lags 8–9 s, and the caveat below
reads that the level has no good limit at all. On the WS the same transaction
takes 17.7 ms, so two fit in a commit and three do not. The ladder does exactly
that:

- lag is 0.16–0.20 s up to 1M, which admits two;
- lag is 2.05 s at 1.5M, which admits three;
- 1M gives 918 tx/s against Run A's 202 at the same lag, and lets 38 expensive
  transactions through per second against Run A's 20.

So the caveat is about the machine and not the workload.

`mix3700` moves the same way and further. Every limit is flat at 0.17–0.24 s
where EPYC's 100K limit lags 7.5 s, and throughput climbs the whole ladder: 167,
211, 297, 612, 1,001. The top limit completes everything offered — 1,001 tx/s,
no cancellations at all, and all 300 expensive transactions per second the mix
contains.

> [!NOTE]
> That limit is bounded by the client rather than by the limit, so where this
> ladder peaks on the WS is not measured here. It is above 200K at this rate.

Taken with `mix20800`, the three ladders say the same thing in three places.
What a limit has room for is a sum of CUs, with no machine in it. Whether the
expensive transactions a commit admits fit inside the commit does depend on
the machine, and that is what decides where each ladder peaks: 37K on EPYC
and above 200K on the WS at 1K/10K, 150K and 500K at 1K/100K, nothing at all
on EPYC and 1M on the WS at 1K/500K.

What each machine actually admits is a separate matter. Where the scheduler
is also working through a deferral backlog it admits less than its limit
allows (finding 3), and the slower machine more so: at `mix3700` with a 10K
limit, EPYC admits 5.4 transactions per commit against the WS's 8.2, while
using only 11 ms of the 50. The two machines match where that pressure is
low — 41 per commit and 821 tx/s on both at `mix20800`/150K — and diverge
where cancellations run past 700/s.

![The three mix ladders on both machines](results/matrix/summary_plots/modes_two_machines.png)

*The three mixes run on both machines, EPYC (orange) against the WS (green): Run
B as the line with markers, that machine's Run A dashed, the shaded band from
the expensive cost to twice it. On every mix the two machines produce the same
numbers while the limit admits work the slower one can still execute, and part
company above it.*

*The answer to the H2 question.* For a shared object whose traffic is mostly
cheap, with some expensive transactions that each take a good part of a commit
interval to execute: set the unit limit at the expensive transaction's cost or
above, but below twice it. One expensive transaction then fits per commit, two
never fit, and the rest of the limit goes to the cheap ones.

Measured at 1K/100K, at 100K–150K CUs:

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
> level's execution time on the machine in question to be set well.

The rule comes with caveats.

- The gain is made of cheap transactions, on a machine that fits only one
  expensive transaction per commit. There the unit limit executes fewer
  expensive transactions than the count limit — 20 per second against 34 at
  `mix20800` on EPYC — and nearly all of its remaining cancellations are
  expensive ones that did not fit: the count limit spreads its cancellations
  over both levels, the unit limit puts them on the expensive one. Whether
  that trade is acceptable depends on what the two levels are worth, which
  the data cannot say. Where the machine fits more than one, the trade
  disappears: on the WS the unit limit executes more of the expensive level
  than the count limit, 38 per second against 20 at `mix50900`/1M and 300
  against 50 at `mix3700`/200K.
- The best limit for a given machine is not a fixed number of CUs. How many
  expensive transactions fit in the interval depends on their cost and on the
  machine: one 100K transaction on EPYC, five on the WS, where 500K serves
  4.6× more of the expensive level at the same throughput and lag; three 10K
  transactions on EPYC at `mix3700`, where the limits that fit only one or two
  fall below the count limit. A limit below twice the expensive cost is safe on
  both machines when one expensive transaction fills most of the interval, and
  on the WS it costs something: at `mix50900` it gives 816 tx/s and 19
  expensive per second, against 918 and 38 one step up. Doing better needs the
  expensive level's execution time on that machine, or a limit expressed in
  time rather than CUs.
- A level whose single transaction overruns the commit interval has no good
  limit on that machine, only a choice between lag and never admitting it. On
  a machine fast enough to fit it, the level behaves like any other: 500K CUs
  has no usable limit on EPYC, and a good one at 1M on the WS.
- The gain exists only where the count limit leaves execution on the object
  idle, which needs the cheap level to be cheap against the drain rate on the
  object.
- The count limit is not the safe default it looks like. At `mix20800` its
  ten admissions hold two 100K transactions on average, 63–65 ms of work per
  50 ms commit, and its lag climbs for a whole 300 s run — at both limits
  its Run B partner held flat. Whether a count of 10 is stable depends on
  the mix, and nothing in the mode lets it be tuned.

---

## H4 — safety (pass/fail)

**PASS.** Across all 2,490 runs of the 124 configurations on EPYC and the 130
runs of the 13 on the WS:

- every safety counter is zero: checkpoint forks
  (`split_brain_checkpoint_forks`, `remote_checkpoint_forks`), inconsistent
  state hash, double-spend attempts, attestation task panics, and soft-lock
  equivocation;
- the per-iteration node state scan found no validator crash, restart, or OOM
  inside any measurement window.

The metric descriptions are in [`../h1/RESULTS.md`](../h1/RESULTS.md), H4 section. The only
run-to-run irregularity anywhere in the grid is the consensus-handler slowdown
of finding 1, which is a performance observation and not a safety failure.

---

## Summary

The takeaway is the TL;DR at the top of this document. Everything behind it:

- per-configuration numbers on EPYC: [`results/matrix/summary.md`](results/matrix/summary.md) and
  [`summary.csv`](results/matrix/summary.csv);
- the same for the WS: [`results/matrix-ws/summary.md`](results/matrix-ws/summary.md);
- figures: `results/matrix/summary_plots/`;
- the cost calibration behind the grid: [`probe-test.md`](probe-test.md).
