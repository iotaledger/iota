# H2 calibration — probe results

The probe (`probe.sh`, swept by `probe_sweep.sh`) characterizes one
`slow::slow(n, size)` point at low rate: the per-transaction computation units
(attested + actual, the values `TotalComputationUnits` schedules on) and the
internal Move-VM execution time. The computation units select the W5 cost points
and set the per-object limits for the H2 mode comparison; see `README.md`.

The grid is a geometric ladder of the product `n·size` (size fixed at 100,
varying n, product 100 → 2M) plus a split-invariance check (product 40000 at
three n/size splits). Each point ran 20 s at 5 QPS (100 transactions), the same
on both machines. The same 21-point sweep ran on two machines:

| machine | CPU | arch | boost | cores |
| --- | --- | --- | --- | --- |
| EPYC | EPYC 9454P | Zen4 | ≈3.8 GHz | 48 |
| WS | Ryzen 9 9950X3D | Zen5 | 5.76 GHz | 16 (+3D V-Cache) |

`compare_machines.py` reproduces the cross-machine table below;
`plot_calibration.py` reproduces the figures.

## How the probe measures (and what changed since the first run)

The client is the `stress` benchmark running **in-docker on the private
network**, submitting **directly to the validators** via the transaction driver
— the attested `submit_tx` path (attestation happens in the validator's
handler regardless of the caller). Every measured value comes from
validator-side Prometheus histograms, pooled over the 4 validators and
differenced over the point's measurement window:

- **Computation units** — `attested_computation_units` /
  `actual_computation_units`, `Δ_sum / Δ_count`. Only attested user
  transactions touch these histograms, and the workload is deterministic, so
  the mean is the exact per-transaction value.
- **Internal execution time** — `authority_state_internal_execution_latency_user`,
  pure post-consensus execution, **user transactions only**. This histogram was
  added for the probe: the pre-existing all-transactions histogram pools the
  network's constant stream of system transactions (commit prologues etc.),
  which outnumber a low-rate workload ~30:1 and drag the mean toward their
  sub-millisecond cost.

Correctness machinery, added after the first calibration run went wrong in
instructive ways:

- The window opens only after Prometheus has scraped the validators, and closes
  only once the user-transaction execution counter stays flat across
  consecutive scrapes — so the window is complete on any hardware, with no
  tuned sleeps.
- A row is recorded only if the window holds ≥400 of the ~412 expected samples
  (100 transactions × 4 validators + client setup), i.e. ≥97 % delivery; an
  under-delivered point fails loudly and is retried instead of writing a
  plausible-looking but polluted row.
- Every sweep starts from a freshly bootstrapped network and tears it down at
  the end, so no run can inherit another run's backlog.

Every row below has exactly 412 samples.

> [!WARNING]
> The July 6 numbers in this file's earlier revision had two defects. (1) The
> execution-time column was pooled from the all-transactions histogram, so it
> mostly averaged system transactions — e.g. the WS ceiling was reported as
> ≈23 ms when the true per-transaction cost is ≈149 ms; the cross-machine
> ratios were likewise computed on diluted values. (2) The EPYC ceiling rows
> were polluted by window contamination. The computation-unit columns were
> exact then and are bit-identical now — only the execution-time analysis is
> superseded by this revision.

---

## Results

### EPYC 9454P

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.610 | 0.034 | 412 |
| 2 | 100 | 200 | 1,000 | 0.654 | 0.016 | 412 |
| 5 | 100 | 500 | 1,000 | 0.918 | 0.055 | 412 |
| 10 | 100 | 1,000 | 1,000 | 1.339 | 0.081 | 412 |
| 20 | 100 | 2,000 | 1,000 | 2.237 | 0.039 | 412 |
| 50 | 100 | 5,000 | 1,000 | 4.797 | 0.108 | 412 |
| 100 | 100 | 10,000 | 3,913 | 9.404 | 0.268 | 412 |
| 200 | 100 | 20,000 | 15,563 | 17.399 | 0.437 | 412 |
| 100 | 400 | 40,000 | 123,330 | 33.937 | 0.473 | 412 |
| 200 | 200 | 40,000 | 124,301 | 32.288 | 0.516 | 412 |
| 400 | 100 | 40,000 | 126,243 | 35.938 | 0.902 | 412 |
| 500 | 100 | 50,000 | 184,495 | 40.862 | 0.909 | 412 |
| 1000 | 100 | 100,000 | 476,728 | 73.439 | 0.994 | 412 |
| 2000 | 100 | 200,000 | 1,060,223 | 108.637 | 3.278 | 412 |
| 5000 | 100 | 500,000 | 2,810,709 | 196.266 | 3.288 | 412 |
| 7000 | 100 | 700,000 | 3,977,699 | 251.242 | 5.208 | 412 |
| 8500 | 100 | 850,000 | 4,854,398 | 272.078 | 5.483 | 412 |
| 10000 | 100 | 1,000,000 | 4,854,398 | 273.460 | 5.430 | 412 |
| 12000 | 100 | 1,200,000 | 4,854,398 | 269.867 | 5.785 | 412 |
| 15000 | 100 | 1,500,000 | 4,854,398 | 271.866 | 5.489 | 412 |
| 20000 | 100 | 2,000,000 | 4,854,398 | 283.513 | 5.051 | 412 |

### WS Ryzen 9 9950X3D

| n | size | product | CU | exec mean (ms) | exec sem (ms) | samples |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 100 | 100 | 1,000 | 0.233 | 0.017 | 412 |
| 2 | 100 | 200 | 1,000 | 0.242 | 0.002 | 412 |
| 5 | 100 | 500 | 1,000 | 0.306 | 0.004 | 412 |
| 10 | 100 | 1,000 | 1,000 | 0.415 | 0.010 | 412 |
| 20 | 100 | 2,000 | 1,000 | 0.583 | 0.018 | 412 |
| 50 | 100 | 5,000 | 1,000 | 1.254 | 0.075 | 412 |
| 100 | 100 | 10,000 | 3,913 | 2.086 | 0.047 | 412 |
| 200 | 100 | 20,000 | 15,563 | 4.030 | 0.089 | 412 |
| 100 | 400 | 40,000 | 123,330 | 7.208 | 0.159 | 412 |
| 200 | 200 | 40,000 | 124,301 | 8.214 | 0.222 | 412 |
| 400 | 100 | 40,000 | 126,243 | 7.487 | 0.178 | 412 |
| 500 | 100 | 50,000 | 184,495 | 9.275 | 0.236 | 412 |
| 1000 | 100 | 100,000 | 476,728 | 18.247 | 0.309 | 412 |
| 2000 | 100 | 200,000 | 1,060,223 | 34.826 | 0.409 | 412 |
| 5000 | 100 | 500,000 | 2,810,709 | 88.812 | 2.141 | 412 |
| 7000 | 100 | 700,000 | 3,977,699 | 126.158 | 2.596 | 412 |
| 8500 | 100 | 850,000 | 4,854,398 | 148.915 | 1.778 | 412 |
| 10000 | 100 | 1,000,000 | 4,854,398 | 146.229 | 1.938 | 412 |
| 12000 | 100 | 1,200,000 | 4,854,398 | 151.727 | 1.702 | 412 |
| 15000 | 100 | 1,500,000 | 4,854,398 | 148.533 | 1.790 | 412 |
| 20000 | 100 | 2,000,000 | 4,854,398 | 148.998 | 1.776 | 412 |

> [!NOTE]
> At the ceiling (product ≥ 850k) every transaction aborts out-of-gas at the
> metering cap; the abort still costs the full execution time, which is what
> the plateau rows measure. Unlike the July run, the ceiling rows are clean on
> both machines (the drain and the delivery guard keep each window complete
> and uncontaminated).

---

## Findings

**1. CUs sit at the floor, rise superlinearly, then hit a hard ceiling.** For
product ≤ 5,000 every point bills at 1,000 — one `gas_rounding_step` — so small
workloads are indistinguishable on cost. From product 10,000 upward CUs rise
steeply (3,913 → 15,563 → … → 2,810,709 at product 500,000), roughly quadratic
in the product at first and flattening toward linear. At the top they hit a
ceiling: product 700k → 3.98M, then 850k through 2M all pin at the *same*
4,854,398 — a five-point plateau. The metered CU flatlines just under the 5M
`max_gas_computation_bucket`, because the VM computation budget is
`min(gas_budget, 5M × gas_price)` — past ≈850k product a transaction can't meter
more, it caps (and aborts out-of-gas) at ≈4.85M. The wide, well-separated CU
range below the ceiling is what gives the mode comparison distinct gas buckets
to calibrate against.

**2. The product is the cost axis; the n/size split barely matters.** At product
40,000 the three splits (100×400, 200×200, 400×100) give CUs within 2.4 %
(123,330 / 124,301 / 126,243) and exec times within ≈11 % on both machines. So
`n·size` sets the cost; more vectors at equal product cost marginally more.
This validates the product as the single W5 cost axis.

![CUs and execution time vs product](results/summary_plots/cu_exec_vs_product.png)

*Top: computation units vs product — one curve, since CUs are
machine-independent; hollow squares are the product-40000 splits, which land on
the curve; the top five rungs (850k–2M) flatline just under the 5M computation
cap (red). Bottom: internal execution time vs product, per machine (both to
product 2M).*

**3. CUs are machine-independent; execution time is not.** Every CU matches to
the digit across both machines — and across node builds and client setups: the
same values came out bit-identical before and after a rebase of the node, and
with the client submitting via the fullnode or directly to validators.
Computation units are protocol-defined gas metering, not wall-clock. Execution
time, in contrast, is the single-threaded Move-VM cost, so it tracks per-core
performance:

| product | n×size | CU | EPYC exec (ms) | WS exec (ms) | WS/EPYC |
| --- | --- | --- | --- | --- | --- |
| 100 | 1×100 | 1,000 | 0.610 | 0.233 | 0.38 |
| 200 | 2×100 | 1,000 | 0.654 | 0.242 | 0.37 |
| 500 | 5×100 | 1,000 | 0.918 | 0.306 | 0.33 |
| 1,000 | 10×100 | 1,000 | 1.339 | 0.415 | 0.31 |
| 2,000 | 20×100 | 1,000 | 2.237 | 0.583 | 0.26 |
| 5,000 | 50×100 | 1,000 | 4.797 | 1.254 | 0.26 |
| 10,000 | 100×100 | 3,913 | 9.404 | 2.086 | 0.22 |
| 20,000 | 200×100 | 15,563 | 17.399 | 4.030 | 0.23 |
| 40,000 | 100×400 | 123,330 | 33.937 | 7.208 | 0.21 |
| 40,000 | 200×200 | 124,301 | 32.288 | 8.214 | 0.25 |
| 40,000 | 400×100 | 126,243 | 35.938 | 7.487 | 0.21 |
| 50,000 | 500×100 | 184,495 | 40.862 | 9.275 | 0.23 |
| 100,000 | 1000×100 | 476,728 | 73.439 | 18.247 | 0.25 |
| 200,000 | 2000×100 | 1,060,223 | 108.637 | 34.826 | 0.32 |
| 500,000 | 5000×100 | 2,810,709 | 196.266 | 88.812 | 0.45 |
| 700,000 | 7000×100 | 3,977,699 | 251.242 | 126.158 | 0.50 |
| 850,000 | 8500×100 | 4,854,398 | 272.078 | 148.915 | 0.55 |
| 1,000,000 | 10000×100 | 4,854,398 | 273.460 | 146.229 | 0.53 |
| 1,200,000 | 12000×100 | 4,854,398 | 269.867 | 151.727 | 0.56 |
| 1,500,000 | 15000×100 | 4,854,398 | 271.866 | 148.533 | 0.55 |
| 2,000,000 | 20000×100 | 4,854,398 | 283.513 | 148.998 | 0.53 |

The WS runs 1.8–4.8× faster per transaction, and the ratio is U-shaped rather
than flat:

- **Fixed-overhead floor** (product ≤ 1,000): ratio ≈ 0.31–0.38 (WS ≈2.6–3.2×
  faster). Exec here is mostly per-tx overhead (≈0.23 ms WS vs ≈0.61 ms EPYC).
- **Compute-bound middle** (product 10k–100k): ratio dips to ≈ 0.21–0.25 (WS
  ≈4–4.8× faster). Raw Move-VM compute dominates and the WS's higher clock,
  newer core, and 3D V-Cache win big — well beyond the ≈1.5× clock ratio alone.
- **Large tail** (product ≥ 500k, CU ≥ 2.8M): ratio climbs back to ≈ 0.45–0.56
  (WS ≈1.8–2.2× faster). Consistent with the working set outgrowing cache and
  the tail becoming memory-bandwidth bound, where the EPYC's many-channel
  server memory competes better and offsets the WS's clock edge. (The U-shape
  is solid; the mechanism is inference.)

At the ceiling both machines are flat and clean: WS ≈146–152 ms, EPYC
≈270–284 ms across all five plateau rungs.

![Execution time vs CUs](results/summary_plots/exec_vs_cu.png)

*Internal execution time vs computation units, per machine. The vertical
cluster at CU = 1,000 is the gas-rounding floor: exec time still rises with the
real work (the product) while the billed CU stays pinned at the floor. The
points piled at CU ≈ 4.85M are the ceiling plateau.*

**4. At ceiling costs, even 5 QPS is heavy load for the attested submit path.**
A byproduct of getting the probe reliable, relevant to the H2/H3 experiment
design. With attestation on, every submitted transaction is executed *twice or
more* on the network's critical path: a full pre-consensus dry-run on the
receiving validator (`attest_transaction`, ≈ the same cost as execution), the
post-consensus execution on every validator, and — when the client waits on a
fullnode — once more during checkpoint replay. At the ceiling that chain is
≈1.5–2 s per transaction on the EPYC, and a closed-loop client whose
back-pressure is a small in-flight budget silently under-delivers once
responses cross its threshold (delivery collapsed to 0–75 of 100 transactions
before the probe moved to direct-to-validator submission, which removes the
fullnode from the response chain). Retries of a submission pay the attestation
dry-run again — there is no attestation cache by digest — so an overloaded
submit path attracts *more* attestation work. For the mode-comparison runs
this means: heavy-CU workloads stress the attested path at rates that look
trivially low on paper, and client throughput must be read together with
delivery counts, never assumed from the target rate.

The takeaway for cross-machine reading: computation units transfer exactly, but
per-transaction latency does not. The EPYC's strength is core count (48c) for
parallel throughput, not per-tx speed — so it lags the high-clock desktop on
anything gated on single-transaction execution, by 2× at the ceiling and up to
almost 5× in the compute-bound middle of the range.
