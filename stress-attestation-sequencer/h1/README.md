# H1 — attestation overhead (V1 vs V2)

End-to-end stress experiment comparing **V1** (attestation OFF) against **V2**
(attestation ON) on a local private network, scraping Prometheus into per-run
JSON, then aggregating many runs into pooled stats and Grafana-style plots.

Shared scripts live one level up in `../`: the network ones (`start.sh`,
`cleanup.sh`, `bootstrap.sh`, `run-stress-docker.sh`) and the two every
experiment uses (`exp_dir.py`, `dump_timeseries.py`). Everything H1-specific is
here.

## Workflow

```bash
# 1. Run an experiment. LABEL is REQUIRED and names the experiment.
#    ITERS (default 1) runs it N times; each adds one iter-NNN to the pool.
LABEL=owned-200qps ITERS=3 WORKLOAD=owned TARGET_QPS=200 ./run.sh

# run.sh then AUTOMATICALLY, across ALL iterations accumulated for the label:
#   - aggregates pooled raw histograms      -> results/<LABEL>/summary.md
#   - renders the H1 figure set (V1 vs V2)   -> results/<LABEL>/plots/
#     (mean/median + variance band; per-validator metrics collapsed to one
#      network curve; uses the .venv)

# Re-plot manually anytime (e.g. different stat/band, or after more iterations):
.venv/bin/python plot.py --label owned-200qps
.venv/bin/python plot.py                        # all labels under results/
.venv/bin/python plot.py --stat mean --band std
```

`run.sh` is the single source of truth for config; pass tunables as env vars
(see the header in `run.sh` for the full list).

## Results layout

Everything for an experiment — data, summary, and plots — lives under its label:

```
results/<LABEL>/
    config.json     # canonical inputs — the WRITE GATE (see below)
    iter-001/       # one run.sh iteration
        run-a-v1-timeseries.json   run-b-v2-timeseries.json
        cleanup.log  bootstrap.log  node-logs/
    iter-002/  ...
    summary.md      # aggregate.py over all iterations
    plots/*.png     # one figure per Grafana panel, V1 vs V2 (plot.py)
```

## The config gate (`../exp_dir.py`)

A `LABEL` is **one experiment = one config**. The first `run.sh` for a label
writes `config.json`; every later run with that label must match it, or it is
**rejected** (`exp_dir.py` prints a diff and exits non-zero). This guarantees a
label's pool is always homogeneous, so aggregating/averaging across its
iterations is valid — and it replaces the old `archive/` de-mixing hack. Use a
**new LABEL** for a different config; `rm -rf results/<LABEL>` to reset one.

## Tooling

- `run.sh` — run the experiment ITERS times; config-gated; auto-aggregates +
  plots.
- `../exp_dir.py` — allocates `iter-NNN` and enforces the config gate. Shared
  with the other experiments; driven entirely by `CFG_*` env vars.
- `../dump_timeseries.py` — scrapes one run window from Prometheus into a raw
  per-run JSON (stdlib only; reset-aware trim). Collects the FULL metric set
  regardless of what's plotted. Re-runnable standalone to re-scrape a past
  window. Shared with the other experiments, so add new metrics here once.
- `aggregate.py` — pools raw histograms across a label's iterations →
  `summary.md`.
- `plot.py` — parses the Grafana dashboard JSON and replays each panel's PromQL
  against the saved series. Per-validator metrics are collapsed to ONE network
  curve (mean/median across validators — not summed), then aggregated across
  iterations (mean/median + IQR/std band), one figure per panel. By default it
  renders the H1 set (Tier 1 attestation-overhead + Tier 2 context, skipping the
  flat Tier-3 safety gates); `--all` renders every panel. Needs the venv:
  `python3 -m venv .venv && .venv/bin/pip install matplotlib numpy`.

Only the scripts are tracked; `.venv/` and `results/` (data + per-label plots)
are gitignored.
