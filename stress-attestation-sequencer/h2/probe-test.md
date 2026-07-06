# H2 calibration — probe results

The probe (`probe.sh`, swept by `probe_sweep.sh`) characterizes one
`slow::slow(n, size)` point at low rate: the per-transaction computation units
(attested + actual, the values `TotalComputationUnits` schedules on) and the
internal Move-VM execution time. The computation units select the W5 cost points
and set the per-object limits for the H2 mode comparison; see `README.md`.

The grid is a geometric ladder of the product `n·size` (size fixed at 100,
varying n, product 100 → 1M) plus a split-invariance check (product 40000 at
three n/size splits). Each point ran 20 s at 5 QPS, giving ~1,700–3,400
execution samples. The same sweep ran on two machines:

| machine | CPU | arch | boost | cores |
| --- | --- | --- | --- | --- |
| EPYC | EPYC 9454P | Zen4 | ~3.8 GHz | 48 |
| WS | Ryzen 9 9950X3D | Zen5 | 5.76 GHz | 16 (+3D V-Cache) |

`compare_machines.py` reproduces the cross-machine table below;
`plot_calibration.py` reproduces the figures.

---

## Results

### EPYC 9454P

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.445 | 0.005 | 2484 |
| 2 | 100 | 200 | 1,000 | 0.464 | 0.005 | 3362 |
| 5 | 100 | 500 | 1,000 | 0.506 | 0.012 | 3270 |
| 10 | 100 | 1,000 | 1,000 | 0.577 | 0.018 | 3229 |
| 20 | 100 | 2,000 | 1,000 | 0.714 | 0.017 | 3319 |
| 50 | 100 | 5,000 | 1,000 | 1.145 | 0.030 | 3257 |
| 100 | 100 | 10,000 | 3,913 | 1.814 | 0.070 | 3351 |
| 200 | 100 | 20,000 | 15,563 | 3.240 | 0.143 | 3340 |
| 100 | 400 | 40,000 | 123,330 | 5.716 | 0.238 | 3310 |
| 200 | 200 | 40,000 | 124,301 | 5.745 | 0.255 | 3337 |
| 400 | 100 | 40,000 | 126,243 | 6.043 | 0.282 | 3261 |
| 500 | 100 | 50,000 | 184,495 | 7.491 | 0.323 | 3232 |
| 1000 | 100 | 100,000 | 476,728 | 12.802 | 0.533 | 3225 |
| 2000 | 100 | 200,000 | 1,060,223 | 18.084 | 1.077 | 3333 |
| 5000 | 100 | 500,000 | 2,810,709 | 32.833 | 1.255 | 3256 |

### WS Ryzen 9 9950X3D

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.211 | 0.005 | 1686 |
| 2 | 100 | 200 | 1,000 | 0.215 | 0.001 | 3136 |
| 5 | 100 | 500 | 1,000 | 0.228 | 0.003 | 3254 |
| 10 | 100 | 1,000 | 1,000 | 0.238 | 0.002 | 3252 |
| 20 | 100 | 2,000 | 1,000 | 0.274 | 0.005 | 3261 |
| 50 | 100 | 5,000 | 1,000 | 0.364 | 0.016 | 3270 |
| 100 | 100 | 10,000 | 3,913 | 0.573 | 0.018 | 3139 |
| 200 | 100 | 20,000 | 15,563 | 0.811 | 0.028 | 3244 |
| 100 | 400 | 40,000 | 123,330 | 1.434 | 0.063 | 3156 |
| 200 | 200 | 40,000 | 124,301 | 1.477 | 0.070 | 3248 |
| 400 | 100 | 40,000 | 126,243 | 1.423 | 0.064 | 3246 |
| 500 | 100 | 50,000 | 184,495 | 1.706 | 0.077 | 3237 |
| 1000 | 100 | 100,000 | 476,728 | 3.027 | 0.117 | 3152 |
| 2000 | 100 | 200,000 | 1,060,223 | 6.048 | 0.255 | 3246 |
| 5000 | 100 | 500,000 | 2,810,709 | 14.496 | 0.731 | 3267 |
| 7000 | 100 | 700,000 | 3,977,699 | 21.518 | 1.148 | 3137 |
| 8500 | 100 | 850,000 | 4,854,398 | 22.711 | 1.106 | 3257 |
| 10000 | 100 | 1,000,000 | 4,854,398 | 22.967 | 1.107 | 3254 |

> [!NOTE]
> The top three WS rungs (product 700k–1M) map the CU ceiling. The EPYC sweep
> was run to product 500k, so the cross-machine comparison below covers the
> shared points; re-run those rungs on EPYC to extend it.

---

## Findings

**1. CUs sit at the floor, rise superlinearly, then hit a hard ceiling.** For
product ≤ 5,000 every point bills at 1,000 — one `gas_rounding_step` — so small
workloads are indistinguishable on cost. From product 10,000 upward CUs rise
steeply (3,913 → 15,563 → … → 2,810,709 at product 500,000), roughly quadratic
in the product at first and flattening toward linear. At the top they hit a
ceiling: product 700k → 3.98M, 850k → 4.85M, and 1M → the *identical* 4.85M. The
metered CU flatlines just under the 5M `max_gas_computation_bucket`, because the
VM computation budget is `min(gas_budget, 5M × gas_price)` — past ~850k product
a transaction can't meter more, it caps (and aborts out-of-gas) at ≈4.85M. The
wide, well-separated CU range below the ceiling is what gives the mode
comparison distinct gas buckets to calibrate against.

**2. The product is the cost axis; the n/size split barely matters.** At product
40,000 the three splits (100×400, 200×200, 400×100) give CUs within 2.4 %
(123,330 / 124,301 / 126,243) and exec times within ~6 %. So `n·size` sets the
cost; more vectors at equal product cost marginally more. This validates the
product as the single W5 cost axis.

![CUs and execution time vs product](results/summary_plots/cu_exec_vs_product.png)

*Top: computation units vs product — one curve, since CUs are
machine-independent; hollow squares are the product-40000 splits, which land on
the curve; the top rungs flatline just under the 5M computation cap (red).
Bottom: internal execution time vs product, per machine (EPYC to 500k, WS to
1M).*

**3. CUs are machine-independent; execution time is not.** Every CU matches to
the digit across both machines — computation units are protocol-defined gas
metering, not wall-clock. Execution time, in contrast, is the single-threaded
Move-VM cost, so it tracks per-core performance:

| product | n×size | CU | EPYC exec (ms) | WS exec (ms) | WS/EPYC |
| --- | --- | --- | --- | --- | --- |
| 100 | 1×100 | 1,000 | 0.445 | 0.211 | 0.47 |
| 200 | 2×100 | 1,000 | 0.464 | 0.215 | 0.46 |
| 500 | 5×100 | 1,000 | 0.506 | 0.228 | 0.45 |
| 1,000 | 10×100 | 1,000 | 0.577 | 0.238 | 0.41 |
| 2,000 | 20×100 | 1,000 | 0.714 | 0.274 | 0.38 |
| 5,000 | 50×100 | 1,000 | 1.145 | 0.364 | 0.32 |
| 10,000 | 100×100 | 3,913 | 1.814 | 0.573 | 0.32 |
| 20,000 | 200×100 | 15,563 | 3.240 | 0.811 | 0.25 |
| 40,000 | 100×400 | 123,330 | 5.716 | 1.434 | 0.25 |
| 40,000 | 200×200 | 124,301 | 5.745 | 1.477 | 0.26 |
| 40,000 | 400×100 | 126,243 | 6.043 | 1.423 | 0.24 |
| 50,000 | 500×100 | 184,495 | 7.491 | 1.706 | 0.23 |
| 100,000 | 1000×100 | 476,728 | 12.802 | 3.027 | 0.24 |
| 200,000 | 2000×100 | 1,060,223 | 18.084 | 6.048 | 0.33 |
| 500,000 | 5000×100 | 2,810,709 | 32.833 | 14.496 | 0.44 |

The WS runs 2.1–4.3× faster per transaction, and the ratio is U-shaped rather
than flat:

- **Fixed-overhead floor** (product ≤ 500): ratio ≈ 0.45–0.47 (WS ~2.1× faster).
  Exec here is mostly per-tx overhead (~0.21 ms WS vs ~0.45 ms EPYC).
- **Compute-bound middle** (product 20k–100k): ratio dips to ≈ 0.23–0.25 (WS
  ~4× faster). Raw Move-VM compute dominates and the WS's higher clock and
  newer core win big.
- **Large tail** (product 500k, CU 2.8M): ratio climbs back to ≈ 0.44.
  Consistent with the working set outgrowing cache and the tail becoming
  memory-bandwidth bound, where the EPYC's many-channel server memory competes
  better and offsets the WS's clock edge. (The U-shape is solid; the mechanism
  is inference.)

![Execution time vs CUs](results/summary_plots/exec_vs_cu.png)

*Internal execution time vs computation units, per machine. The vertical cluster
at CU = 1,000 is the gas-rounding floor: exec time still rises with the real
work (the product) while the billed CU stays pinned at the floor.*

The takeaway for cross-machine reading: computation units transfer exactly, but
per-transaction latency does not. The EPYC's strength is core count (48c) for
parallel throughput, not per-tx speed — so it lags the high-clock desktop on
anything gated on single-transaction execution (e.g. checkpoint-creation lag at
low concurrency).
