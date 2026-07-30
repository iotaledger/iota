# H2 calibration — probe results

The probe (`probe.sh`, swept by `probe_sweep.sh`) measures one
`slow::slow(n, size)` point at a low rate. For each point, it records the
per-transaction computation units — **attested**, metered during the attestation
dry-run and the value `TotalComputationUnits` uses for scheduling, and **actual**,
metered at post-consensus execution — plus the Move VM execution time. The
workload is the owned-object form of `slow` (W4 in `../stress-plan.md`), so
attested and actual computation units should be equal, because no state can change
between the dry-run and execution. *Sizing the `TotalComputationUnits` limit*
below covers what the numbers are used for; `README.md` has the run plan.

The sweep steps the product `n × size` geometrically (`size` fixed at 100, varying
`n`, product 100 → 2M), then adds three points that hold the product at 40000
while changing how it divides between `n` and `size`. Each point ran 20 s at
5 QPS, so 100 transactions. The same 21 points ran on two machines:

| machine | CPU | arch | boost | cores |
| --- | --- | --- | --- | --- |
| EPYC | EPYC 9454P | Zen4 | ≈3.8 GHz | 48 |
| WS | Ryzen 9 9950X3D | Zen5 | 5.76 GHz | 16 (+3D V-Cache) |

`compare_machines.py` reproduces the cross-machine table below;
`plot_calibration.py` reproduces the figures.

## How the probe measures

The client is the `stress` benchmark running in-docker on the private network,
submitting *directly to the validators* via the transaction driver (the
attested `submit_tx` path). Every measured value comes from validator-side
Prometheus histograms, pooled over the 4 validators and differenced over the
point's measurement window:

- **Computation units** — `attested_computation_units` and
  `actual_computation_units`, as `Δ_sum / Δ_count`. Only attested user
  transactions reach these histograms, and the workload is deterministic (100
  identical owned-object transactions), so the mean is the exact
  per-transaction value.
- **Execution time** — `authority_state_internal_execution_latency_user`:
  post-consensus execution, user transactions only. This histogram was added for
  the probe, because the existing all-transactions one also counts the network's
  steady stream of system transactions (commit prologues and similar). Those
  outnumber a low-rate workload roughly 30 to 1 and, being sub-millisecond, pull
  the mean down.

The measurement window is anchored at the exact instant spamming starts — the
client prints that timestamp when its warmup ends — so the delta excludes
the gas coin setup transactions, which run during warmup. Without this, the
few cheap setup transactions (at the 1,000-CU floor) pool into the mean and
bias it low. The client also waits 2 s between warmup and spamming so the
baseline sits in a quiet gap. Every row below has exactly 400 samples: 100
workload transactions executed on each of the 4 validators.

---

## Results

### EPYC 9454P

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.545 | 0.012 | 400 |
| 2 | 100 | 200 | 1,000 | 0.637 | 0.010 | 400 |
| 5 | 100 | 500 | 1,000 | 0.912 | 0.055 | 400 |
| 10 | 100 | 1,000 | 1,000 | 1.342 | 0.083 | 400 |
| 20 | 100 | 2,000 | 1,000 | 2.237 | 0.038 | 400 |
| 50 | 100 | 5,000 | 1,000 | 4.951 | 0.108 | 400 |
| 100 | 100 | 10,000 | 4,000 | 9.278 | 0.235 | 400 |
| 200 | 100 | 20,000 | 16,000 | 18.776 | 0.478 | 400 |
| 100 | 400 | 40,000 | 127,000 | 36.411 | 0.577 | 400 |
| 200 | 200 | 40,000 | 128,000 | 35.137 | 0.543 | 400 |
| 400 | 100 | 40,000 | 130,000 | 35.037 | 0.644 | 400 |
| 500 | 100 | 50,000 | 190,000 | 42.726 | 0.865 | 400 |
| 1000 | 100 | 100,000 | 491,000 | 74.615 | 0.805 | 400 |
| 2000 | 100 | 200,000 | 1,092,000 | 111.961 | 3.071 | 400 |
| 5000 | 100 | 500,000 | 2,895,000 | 204.800 | 3.125 | 400 |
| 7000 | 100 | 700,000 | 4,097,000 | 266.155 | 5.098 | 400 |
| 8500 | 100 | 850,000 | 5,000,000 | 283.822 | 4.588 | 400 |
| 10000 | 100 | 1,000,000 | 5,000,000 | 282.936 | 4.620 | 400 |
| 12000 | 100 | 1,200,000 | 5,000,000 | 287.358 | 4.410 | 400 |
| 15000 | 100 | 1,500,000 | 5,000,000 | 285.204 | 4.679 | 400 |
| 20000 | 100 | 2,000,000 | 5,000,000 | 287.840 | 4.665 | 400 |

### WS Ryzen 9 9950X3D

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.227 | 0.001 | 400 |
| 2 | 100 | 200 | 1,000 | 0.223 | 0.001 | 400 |
| 5 | 100 | 500 | 1,000 | 0.340 | 0.006 | 400 |
| 10 | 100 | 1,000 | 1,000 | 0.403 | 0.009 | 400 |
| 20 | 100 | 2,000 | 1,000 | 0.700 | 0.017 | 400 |
| 50 | 100 | 5,000 | 1,000 | 1.315 | 0.074 | 400 |
| 100 | 100 | 10,000 | 4,000 | 2.297 | 0.035 | 400 |
| 200 | 100 | 20,000 | 16,000 | 4.274 | 0.090 | 400 |
| 100 | 400 | 40,000 | 127,000 | 7.650 | 0.172 | 400 |
| 200 | 200 | 40,000 | 128,000 | 7.923 | 0.190 | 400 |
| 400 | 100 | 40,000 | 130,000 | 7.957 | 0.198 | 400 |
| 500 | 100 | 50,000 | 190,000 | 8.987 | 0.194 | 400 |
| 1000 | 100 | 100,000 | 491,000 | 18.813 | 0.319 | 400 |
| 2000 | 100 | 200,000 | 1,092,000 | 35.973 | 0.302 | 400 |
| 5000 | 100 | 500,000 | 2,895,000 | 94.244 | 2.112 | 400 |
| 7000 | 100 | 700,000 | 4,097,000 | 137.892 | 1.855 | 400 |
| 8500 | 100 | 850,000 | 5,000,000 | 154.762 | 1.012 | 400 |
| 10000 | 100 | 1,000,000 | 5,000,000 | 154.992 | 1.000 | 400 |
| 12000 | 100 | 1,200,000 | 5,000,000 | 156.435 | 0.928 | 400 |
| 15000 | 100 | 1,500,000 | 5,000,000 | 150.103 | 1.245 | 400 |
| 20000 | 100 | 2,000,000 | 5,000,000 | 154.293 | 1.035 | 400 |

> [!NOTE]
> At the ceiling (product ≥ 850k), every transaction exhausts its gas budget and
> aborts out of gas. The abort still costs the full execution time, which is what
> those rows measure.

---

## Findings

**1. Computation units sit at a floor, then rise steeply, then stop at a
ceiling.** For product ≤ 5,000 every point is charged 1,000 — one
`gas_rounding_step` — so light workloads cannot be told apart by computation cost.
From product 10,000 upward, the charge rises fast (4,000 → 16,000 → … → 2,895,000
at product 500,000). The steepest stretch is between products 20k and 40k, where
doubling the product multiplies the charge by 8; growth flattens toward linear
above that. At the top, it stops: product 700k gives 4,097,000, and 850k through
2M all give exactly 5,000,000. That 5,000,000 is the transaction's gas budget
expressed in computation units (the 5-IOTA budget divided by the 1,000 gas price).
Once the work would cost more than the budget covers, the transaction aborts out
of gas and is charged the whole budget, so every point past that reports the same
figure. It happens to match the 5M `max_gas_computation_bucket`, but the gas
budget is what binds here. The wide range below the ceiling is what gives the mode
comparison distinct gas buckets to calibrate against.

**2. The product drives the cost; how it divides between `n` and `size` barely
matters.** At product 40,000 the three divisions (100×400, 200×200, 400×100) give
computation units within 2.4 % of each other (127,000 / 128,000 / 130,000) and
execution times within about 4 % on both machines. So `n × size` sets the cost,
with more vectors at the same product costing marginally more. The product alone
is therefore enough to describe the workload's cost.

![CUs and execution time vs product](results/summary_plots/cu_exec_vs_product.png)

*Top: computation units vs product — one curve, since CUs are
machine-independent; the square markers are the product-40000 splits, which
land on the curve; the top five points (850k–2M) sit exactly on the 5M
gas-budget cap (red). Bottom: internal execution time vs product, per machine
(both to product 2M).*

**3. CUs are machine-independent; execution time is not.** Every CU matches to
the digit across both machines — computation units are protocol-defined gas
metering, not wall-clock. Execution time, in contrast, is the single-threaded
Move-VM cost, so it tracks per-core performance:

| product | n×size | CU | EPYC exec (ms) | WS exec (ms) | WS/EPYC |
| --- | --- | --- | --- | --- | --- |
| 100 | 1×100 | 1,000 | 0.545 | 0.227 | 0.42 |
| 200 | 2×100 | 1,000 | 0.637 | 0.223 | 0.35 |
| 500 | 5×100 | 1,000 | 0.912 | 0.340 | 0.37 |
| 1,000 | 10×100 | 1,000 | 1.342 | 0.403 | 0.30 |
| 2,000 | 20×100 | 1,000 | 2.237 | 0.700 | 0.31 |
| 5,000 | 50×100 | 1,000 | 4.951 | 1.315 | 0.27 |
| 10,000 | 100×100 | 4,000 | 9.278 | 2.297 | 0.25 |
| 20,000 | 200×100 | 16,000 | 18.776 | 4.274 | 0.23 |
| 40,000 | 100×400 | 127,000 | 36.411 | 7.650 | 0.21 |
| 40,000 | 200×200 | 128,000 | 35.137 | 7.923 | 0.23 |
| 40,000 | 400×100 | 130,000 | 35.037 | 7.957 | 0.23 |
| 50,000 | 500×100 | 190,000 | 42.726 | 8.987 | 0.21 |
| 100,000 | 1000×100 | 491,000 | 74.615 | 18.813 | 0.25 |
| 200,000 | 2000×100 | 1,092,000 | 111.961 | 35.973 | 0.32 |
| 500,000 | 5000×100 | 2,895,000 | 204.800 | 94.244 | 0.46 |
| 700,000 | 7000×100 | 4,097,000 | 266.155 | 137.892 | 0.52 |
| 850,000 | 8500×100 | 5,000,000 | 283.822 | 154.762 | 0.55 |
| 1,000,000 | 10000×100 | 5,000,000 | 282.936 | 154.992 | 0.55 |
| 1,200,000 | 12000×100 | 5,000,000 | 287.358 | 156.435 | 0.54 |
| 1,500,000 | 15000×100 | 5,000,000 | 285.204 | 150.103 | 0.53 |
| 2,000,000 | 20000×100 | 5,000,000 | 287.840 | 154.293 | 0.54 |

The WS runs 1.8–4.8× faster per transaction, and the ratio is U-shaped rather
than flat:

- **Small products** (≤ 2,000): ratio ≈ 0.30–0.42 (WS ≈2.4–3.3× faster).
  Execution here is mostly per-transaction overhead (≈0.23 ms WS vs ≈0.55 ms
  EPYC).
- **Middle of the range** (product 10k–100k): ratio dips to ≈ 0.21–0.25 (WS
  ≈4–4.8× faster). Raw Move VM compute dominates, and the WS's higher clock,
  newer core, and 3D V-Cache gain the most here — well beyond the ≈1.5× clock
  ratio alone.
- **Large products** (≥ 500k, CU ≥ 2.9M): ratio climbs back to ≈ 0.46–0.55 (WS
  ≈1.8–2.2× faster). Consistent with the working set outgrowing cache and the
  tail becoming memory-bandwidth bound, where the EPYC's many-channel server
  memory competes better and offsets the WS's clock edge. (The U-shape is solid;
  the explanation is a guess.)

At the ceiling, both machines are flat: WS ≈150–156 ms, EPYC ≈283–288 ms across
all five plateau points.

![Execution time vs CUs](results/summary_plots/exec_vs_cu.png)

*Internal execution time vs computation units, per machine. The vertical
cluster at CU = 1,000 is the gas-rounding floor: execution time still rises
with the real work (the product) while the billed CU stays pinned at the floor.
The points piled at CU = 5M are the ceiling plateau.*

So when reading results across machines: computation units transfer exactly, but
per-transaction execution time does not. The EPYC's strength is core count (48c)
for parallel throughput, not per-transaction speed — so it lags the high-clock
desktop on anything that depends on a single transaction's execution, by ≈1.8× at
the ceiling and up to ≈4.8× in the compute-bound middle of the range.

---

## Sizing the `TotalComputationUnits` limit

This is what the calibration is for. Production runs per-object congestion
control in `TotalTxCount` mode with a base limit of 10 and an overshoot of 100
per object per commit (`max_accumulated_txn_cost_per_object_in_mysticeti_commit`
= 10, `max_congestion_limit_overshoot_per_commit` = 100). That mode counts every
transaction as 1, ignoring cost: a per-object commit admits the same 10 (+100
burst) transactions whether each costs 1,000 CU or 5,000,000 CU — the same count
covering a 5,000× difference in real work.

`TotalComputationUnits` limits on attested cost instead of count. The question H2
answers is which CU limit to give it. Mapping
today's count limits onto the CU scale means multiplying by the per-transaction
cost — but the calibration shows that cost spans 1,000 → 5,000,000 CU, so the
equivalent limit spans the same 5,000×:

| CU per tx | base limit (×10) | overshoot (×100) |
| --- | --- | --- |
| 1,000 | 10,000 | 100,000 |
| 4,000 | 40,000 | 400,000 |
| 16,000 | 160,000 | 1,600,000 |
| 127,000 | 1,270,000 | 12,700,000 |
| 128,000 | 1,280,000 | 12,800,000 |
| 130,000 | 1,300,000 | 13,000,000 |
| 190,000 | 1,900,000 | 19,000,000 |
| 491,000 | 4,910,000 | 49,100,000 |
| 1,092,000 | 10,920,000 | 109,200,000 |
| 2,895,000 | 28,950,000 | 289,500,000 |
| 4,097,000 | 40,970,000 | 409,700,000 |
| 5,000,000 | 50,000,000 | 500,000,000 |

Both ends of that range are unusable:

- **Lower bound** — size the limit for all-light traffic (1,000 CU): base
  10,000, overshoot 100,000 CU. But 10,000 CU is smaller than a single heavy
  transaction (5,000,000 CU), so not even one heavy transaction fits per object
  per commit — heavy traffic is deferred indefinitely.
- **Upper bound** — size it for all-heavy traffic (5,000,000 CU): base
  50,000,000, overshoot 500,000,000 CU. That admits 50,000 light transactions
  (1,000 CU each) per object per commit — 5,000× today's 10, i.e. effectively
  no throttling of light traffic.

So the workable limit sits between, and where exactly depends on the workload
mix. Choosing and justifying it is the H2 mode comparison: run `TotalTxCount`
(base 10, overshoot 100) against `TotalComputationUnits` at candidate CU limits
from this range, on W1 (shared-counter, uniform light cost) and W5 (slow,
variable cost), and compare throughput, latency, and per-object deferral.
