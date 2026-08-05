#!/usr/bin/env python3
"""aggregate.py — shared aggregation machinery for h1/h2 aggregate.py.

Statistics over the raw timeseries JSONs that dump_timeseries.py writes:
cumulative-counter deltas, histogram-bucket pooling across runs and the
Prometheus-style quantile over the pooled buckets. Percentiles are computed
by POOLING the raw histogram buckets across runs — the statistically correct
way to combine percentiles (you cannot average per-run quantiles).

What each experiment measures and how its summary.md is laid out stays in the
experiment's own aggregate.py; only the experiment-agnostic pieces live here.
Pure stdlib, like dump_timeseries.py.
"""

import glob
import json
import math
import os
import sys


def delta(values):
    """Increment of a cumulative counter series over its window (last - first)."""
    if not values:
        return 0.0
    first, last = float(values[0][1]), float(values[-1][1])
    d = last - first
    return d if d >= 0 else last  # counter reset within the window: fall back to last


_warned_error_series = set()


def series_list(series, name):
    """A metric's series list from one run's `series` dict, tolerating scrape
    failures: dump_timeseries stores `{"error": "..."}` when a Prometheus query
    failed, which is not iterable as series. Warn once per metric and treat it
    as no data, so one bad scrape does not abort aggregation of a whole label."""
    v = series.get(name, [])
    if isinstance(v, list):
        return v
    if name not in _warned_error_series:
        _warned_error_series.add(name)
        err = v.get("error", v) if isinstance(v, dict) else v
        print(
            f"WARN: metric '{name}' has a failed scrape ({err}); treating as no data",
            file=sys.stderr,
        )
    return []


def series_max(series_per_run, metric):
    """Max value of a metric across all series and runs — a robust 'ever non-zero'
    test for safety counters (counters: final count; gauges: transient flip to 1)."""
    m = 0.0
    for s in series_per_run:
        for x in series_list(s, metric):
            for _, v in x.get("values", []):
                try:
                    m = max(m, float(v))
                except (TypeError, ValueError):
                    pass
    return m


def source_total(series_per_run, key, source):
    """Total increment of a labeled counter for one `source`, summed across hosts
    and runs — the count of rejections attributed to that overload source."""
    total = 0.0
    for series in series_per_run:
        for s in series_list(series, key):
            if s.get("metric", {}).get("source") == source:
                total += delta(s.get("values", []))
    return total


def pooled_buckets(series_per_run, base):
    """Sum per-`le` bucket increments across hosts AND runs -> {le: count}.

    This is the pooled histogram: equivalent to PromQL `sum by (le) (...)` but
    combined over every run, so the quantile is taken on the union of samples.
    """
    acc = {}
    for series in series_per_run:
        for s in series_list(series, f"{base}_bucket"):
            le = s.get("metric", {}).get("le")
            if le is None:
                continue
            acc[le] = acc.get(le, 0.0) + delta(s.get("values", []))
    return acc


def hquantile(q, buckets):
    """Prometheus-style histogram_quantile over cumulative {le: count}."""
    if not buckets:
        return None
    pts = sorted(
        (math.inf if le in ("+Inf", "Inf", "inf") else float(le), c)
        for le, c in buckets.items()
    )
    total = pts[-1][1]  # +Inf cumulative == total count
    if total <= 0:
        return None
    rank = q * total
    prev_le, prev_c = 0.0, 0.0
    for le, c in pts:
        if c >= rank:
            if math.isinf(le):
                return prev_le if prev_le > 0 else None
            if c == prev_c:
                return le
            return prev_le + (le - prev_le) * (rank - prev_c) / (c - prev_c)
        prev_le, prev_c = le, c
    return pts[-1][0]


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def load(group_glob):
    """Load every timeseries JSON matching the glob, skipping unreadable ones."""
    runs = []
    for path in sorted(glob.glob(group_glob)):
        try:
            runs.append(json.load(open(path)))
        except Exception as e:  # noqa: BLE001
            print(f"WARN: skipping {path}: {e}", file=sys.stderr)
    return runs


def configs(runs):
    """Distinct config dicts across the pooled runs (to flag mixed pools)."""
    seen = []
    for r in runs:
        c = r.get("config", {})
        if c and c not in seen:
            seen.append(c)
    return seen


def crash_incidents(results_dir, run_dirs):
    """Scan per-iteration _state.log for a validator that crashed, restarted, or
    was OOM-killed — a safety failure the timeseries counters don't capture.
    run.sh writes one line per node:
      /validator-1 status=running restarts=0 oom=false exit=0
    `run_dirs` maps each run's node-log subdir to its display tag, e.g.
    (("run-a-node-logs", "A"), ("run-b-node-logs", "B")). Returns human-readable
    strings for any non-clean node, tagged with the run it belongs to."""
    incidents = []
    for subdir, run in run_dirs:
        for sp in sorted(
            glob.glob(os.path.join(results_dir, "*", subdir, "_state.log"))
        ):
            itr = sp.split(os.sep)[-3]  # results/<LABEL>/<iter-NNN>/<subdir>/_state.log
            try:
                lines = open(sp).read().splitlines()
            except Exception as e:  # noqa: BLE001
                print(f"WARN: cannot read {sp}: {e}", file=sys.stderr)
                continue
            for line in lines:
                toks = line.split()
                if not toks:
                    continue
                kv = dict(t.split("=", 1) for t in toks if "=" in t)
                restarts = int(kv.get("restarts", "0") or 0)
                oom = kv.get("oom", "false") == "true"
                status = kv.get("status", "")
                if restarts > 0 or oom or status not in ("running", ""):
                    incidents.append(
                        f"[{run}] {itr} {toks[0]}: status={status} restarts={restarts} oom={oom}"
                    )
    return incidents


def fmt(x):
    return "—" if x is None else f"{x:.6g}"


def dlt(a, b):
    return "—" if (a is None or b is None) else f"{b - a:+.6g}"
