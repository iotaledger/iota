# Stress-test runs and results

Each hypothesis has its own results document:

- [h1/RESULTS.md](h1/RESULTS.md) — H1, attestation overhead. Attestation off
  vs on under the same load, owned-object `slow` workload (W4), on 4 and 24
  validators.
- [h2/RESULTS.md](h2/RESULTS.md) — H2, congestion-control mode comparison.
  `TotalTxCount` vs `TotalComputationUnits`, shared-object `slow` workload
  (W5). 80 configurations with one value of computation units per run, 25
  that mix computation units per run, and 4 of those rerun on a second
  machine.

Both documents have the same structure: goal, TL;DR, setup, findings, safety
check (H4), summary. The test plan is in `stress-plan.md`.
