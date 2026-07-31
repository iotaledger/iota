#!/usr/bin/env python3
"""probe_scrape.py — read one probe window from Prometheus and report the
per-transaction computation units and internal execution time for a single
`slow::slow(n, size)` point.

The H2 calibration pre-step needs, per workload, the attested computation units
per transaction (the value `TotalComputationUnits` schedules on) so the two
congestion modes can be given the same effective per-object capacity. This
scrapes three histograms over the probe's `[start, end]` window, pools them
across validators, and derives:

  - computation units: mean = Δ_sum / Δ_count. The workload is deterministic, so
    every transaction is identical and this mean IS the exact per-transaction
    value (`_sum` accumulates exact values; no bucket rounding). Reported for
    both `attested_computation_units` and `actual_computation_units`; for
    owned-object txs they should match (shared-object state could change
    between the dry-run and execution).
  - internal execution time (`authority_state_internal_execution_latency_user`,
    pure post-consensus VM execution, user transactions only): mean =
    Δ_sum / Δ_count (exact), std estimated from the histogram bucket deltas
    (bucket-resolution), sem = std / sqrt(N).

Pure stdlib (urllib/json) — runs on system python3, no venv. Mirrors the
query/reset-trim approach of ../dump_timeseries.py.

Usage:  probe_scrape.py <start_epoch> <end_epoch> <step_s> <csv_out>
Env:    PROM (Prometheus base URL, default http://localhost:9090)
        CFG_* recorded as metadata columns in the CSV row (slow_n, slow_size,
        product, shared, qps, duration).
Exit:   non-zero if no execution samples were seen in the window.
"""

import csv
import json
import math
import os
import sys
import urllib.parse
import urllib.request

# Seconds; pure post-consensus execution, USER transactions only. The plain
# (non-_user) histogram pools system transactions (commit prologues etc.) that
# run continuously at ~100/s across the network and dwarf the ~500 spam
# observations, dragging the mean toward their ~0.2 ms cost.
EXEC = "authority_state_internal_execution_latency_user"
ATTESTED = "attested_computation_units"  # scheduling input (gas units)
ACTUAL = "actual_computation_units"  # measured after execution (gas units)
# Seconds; consensus commit timestamp -> local checkpoint built. The builder
# waits for the commit's transactions to execute before building, so this lag
# includes the execution wait — heavier txs in a commit raise it. At low rate a
# checkpoint holds ~1 workload tx and many are system-only, so the mean is
# diluted by empty checkpoints; the tail (p95/p99) is the loaded-checkpoint
# signal. Supplementary — not gated, unlike the execution-sample count.
CKPT = "checkpoint_creation_latency"

start, end, step, csv_out = sys.argv[1:5]
prom = os.environ.get("PROM", "http://localhost:9090")
cfg = {k[4:]: v for k, v in os.environ.items() if k.startswith("CFG_")}


# Only validator series: the fullnode re-executes everything during checkpoint
# sync and would add a 5th sample per transaction; the calibration measures
# validator-side execution.
JOBS = '{job=~"Validator_.*"}'


def query_range(q):
    url = (
        prom
        + "/api/v1/query_range?"
        + urllib.parse.urlencode({"query": q, "start": start, "end": end, "step": step})
    )
    with urllib.request.urlopen(url, timeout=60) as r:
        return json.load(r).get("data", {}).get("result", [])


def trim_after_last_reset(values):
    """Drop samples up to and including the last counter reset in the window, so
    last-first over the kept samples is this process's in-window increase (as
    PromQL increase() would compute). See ../dump_timeseries.py."""
    last = 0
    for i in range(1, len(values)):
        if float(values[i][1]) < float(values[i - 1][1]):
            last = i
    return values[last:]


def delta(series):
    """Sum of per-series in-window increase (last - first, reset-aware) over all
    series returned for a counter/histogram-component selector."""
    total = 0.0
    for s in series:
        vals = trim_after_last_reset(s.get("values", []))
        if len(vals) >= 1:
            total += float(vals[-1][1]) - float(vals[0][1])
    return total


def hist_mean_count(base):
    """(Δ_sum, Δ_count) pooled across validator series for a histogram base."""
    return (
        delta(query_range(f"{base}_sum{JOBS}")),
        delta(query_range(f"{base}_count{JOBS}")),
    )


def bucket_deltas(base):
    """Pooled non-cumulative per-bucket counts as [(rep_value, count), ...].

    Cumulative bucket counts are summed across hosts per `le`, differenced
    between adjacent `le` to get each bucket's count, and each bucket is
    represented by the midpoint of its (lower, upper] edge. The unbounded +Inf
    bucket is represented by the last finite `le` (best available)."""
    by_le = {}
    for s in query_range(f"{base}_bucket{JOBS}"):
        le = s["metric"].get("le")
        if le is None:
            continue
        by_le[le] = by_le.get(le, 0.0) + delta([s])
    edges = sorted(by_le, key=lambda x: float("inf") if x == "+Inf" else float(x))
    out, prev_cum, prev_le = [], 0.0, 0.0
    for le in edges:
        cum = by_le[le]
        cnt = cum - prev_cum
        if le == "+Inf":
            rep = prev_le
        else:
            rep = (prev_le + float(le)) / 2.0
            prev_le = float(le)
        if cnt > 0:
            out.append((rep, cnt))
        prev_cum = cum
    return out


def exec_stats():
    """(mean_ms, std_ms, sem_ms, n) for the internal execution latency."""
    d_sum, d_count = hist_mean_count(EXEC)
    n = int(round(d_count))
    if n <= 0:
        return None
    mean = d_sum / d_count  # seconds
    var = 0.0
    for rep, cnt in bucket_deltas(EXEC):
        var += cnt * (rep - mean) ** 2
    var = max(var / d_count, 0.0)
    std = math.sqrt(var)
    sem = std / math.sqrt(d_count)
    return mean * 1e3, std * 1e3, sem * 1e3, n


def cu_mean(base):
    """Exact per-transaction computation units (deterministic workload), or None."""
    d_sum, d_count = hist_mean_count(base)
    return (d_sum / d_count) if d_count > 0 else None


def quantile(pairs, q):
    """q-th quantile over [(rep_value, count), ...] at bucket resolution."""
    total = sum(c for _, c in pairs)
    if total <= 0:
        return None
    target, cum = q * total, 0.0
    for rep, c in sorted(pairs):
        cum += c
        if cum >= target:
            return rep
    return pairs[-1][0]


def ckpt_stats():
    """(mean_ms, p50_ms, p95_ms, p99_ms, n) for checkpoint creation lag, or None.
    Pooled across validators, so n counts each checkpoint once per validator."""
    d_sum, d_count = hist_mean_count(CKPT)
    n = int(round(d_count))
    if n <= 0:
        return None
    b = bucket_deltas(CKPT)
    return (
        d_sum / d_count * 1e3,
        (quantile(b, 0.50) or 0.0) * 1e3,
        (quantile(b, 0.95) or 0.0) * 1e3,
        (quantile(b, 0.99) or 0.0) * 1e3,
        n,
    )


ex = exec_stats()
if ex is None:
    print(
        "probe_scrape: no execution samples in the window — was any transaction "
        "executed? (check the stress log and that attestation is ON)",
        file=sys.stderr,
    )
    sys.exit(1)
exec_mean_ms, exec_std_ms, exec_sem_ms, n = ex

# The stress client sometimes fails to sustain its target rate (down to zero
# successful transactions on a bad point); the window would then hold far fewer
# samples than the workload and must not land in the CSV. 100 workload txs
# executed on 4 validators give 400 samples (the warmup/setup txs are excluded
# by the window baseline); require the full 400 so any under-delivery is refused
# and the sweep marks it FAILED.
min_samples = int(os.environ.get("MIN_SAMPLES", "400"))
if n < min_samples:
    print(
        f"probe_scrape: only {n} execution samples (< {min_samples}) — the "
        "stress client under-delivered (check its report in the point log); "
        "re-run this point. Row NOT appended.",
        file=sys.stderr,
    )
    sys.exit(1)
attested = cu_mean(ATTESTED)
actual = cu_mean(ACTUAL)
ckpt = ckpt_stats()


def fmt(x, unit=""):
    return "n/a" if x is None else f"{x:.1f}{unit}"


print(
    f"slow(n={cfg.get('slow_n', '?')}, size={cfg.get('slow_size', '?')})  "
    f"product={cfg.get('product', '?')}  shared={cfg.get('shared', '?')}  "
    f"qps={cfg.get('qps', '?')}  N={n}"
)
print(
    f"  computation units : attested={fmt(attested)}  actual={fmt(actual)} (gas units)"
)
print(
    f"  internal exec time: {exec_mean_ms:.2f} ms ± {exec_sem_ms:.2f} (sem)  "
    f"[std {exec_std_ms:.2f} ms, N={n}]"
)
# User txs per checkpoint = user executions / checkpoints, both pooled over the
# same validators so the ratio is per-validator. There is no user-only
# per-checkpoint metric (transactions_included_in_checkpoint etc. count system
# txs too), and this is the loading diagnostic for tuning QPS: aim for a few.
user_per_ckpt = n / ckpt[4] if (ckpt is not None and ckpt[4] > 0) else None
if ckpt is not None:
    cmean, cp50, cp95, cp99, cn = ckpt
    print(
        f"  checkpoint lag    : mean={cmean:.1f}  p50={cp50:.0f}  p95={cp95:.0f}  "
        f"p99={cp99:.0f} ms  (n_ckpt={cn}, pooled over validators)"
    )
    print(f"  user txs/ckpt     : {user_per_ckpt:.2f}  (loading check — aim for a few)")
else:
    print("  checkpoint lag    : n/a (no checkpoints created in the window)")

row = {
    # `start` may be a float epoch (the spam-start marker) or an int; store as int seconds.
    "start_epoch": int(float(start)),
    "slow_n": cfg.get("slow_n", ""),
    "slow_size": cfg.get("slow_size", ""),
    "product": cfg.get("product", ""),
    "shared": cfg.get("shared", ""),
    "qps": cfg.get("qps", ""),
    "duration": cfg.get("duration", ""),
    "n_samples": n,
    "attested_cu": "" if attested is None else round(attested, 3),
    "actual_cu": "" if actual is None else round(actual, 3),
    "exec_mean_ms": round(exec_mean_ms, 4),
    "exec_std_ms": round(exec_std_ms, 4),
    "exec_sem_ms": round(exec_sem_ms, 4),
    "ckpt_lag_mean_ms": "" if ckpt is None else round(ckpt[0], 3),
    "ckpt_lag_p50_ms": "" if ckpt is None else round(ckpt[1], 3),
    "ckpt_lag_p95_ms": "" if ckpt is None else round(ckpt[2], 3),
    "ckpt_lag_p99_ms": "" if ckpt is None else round(ckpt[3], 3),
    "ckpt_n": "" if ckpt is None else ckpt[4],
    "user_txs_per_ckpt": "" if user_per_ckpt is None else round(user_per_ckpt, 3),
}
new_file = not os.path.exists(csv_out)
os.makedirs(os.path.dirname(csv_out) or ".", exist_ok=True)
with open(csv_out, "a", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(row))
    if new_file:
        w.writeheader()
    w.writerow(row)
print(f"  -> appended row to {csv_out}")
