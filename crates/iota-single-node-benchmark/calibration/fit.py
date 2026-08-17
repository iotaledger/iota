#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""Fit cpu_time coefficients from sweep datasets (Stage 3 of Phase 2).

Model: measured_ns = c0 + sum_j w_j * count_j, fit with non-negative least
squares over the per-transaction rows of one or more sweep.py datasets.

    ./fit.py --data DIR [DIR ...] --out calibration-artifact.json

Outputs the versioned calibration artifact: fitted constants, the safety
multiplier `m` chosen on a held-out split (smallest m with m*predicted >=
measured on >= the coverage target), the separability check for the three
interpreter components, comparisons against each sweep's single-knob slope
anchor, and the source datasets' machine manifests.

Stdlib only; the NNLS solver is Lawson-Hanson on the Gram matrix, exact
enough at this scale (tens of predictors, tens of thousands of rows).
"""

import argparse
import json
import statistics
import sys
import time
from pathlib import Path

# Predictors, in the plan's dimension order. Native cost is priced per
# function: each `native_gas_by_function` key becomes a gas column and a
# call-count column. Two columns because (calls, gas) spans the same space
# as (per-call cost, per-byte cost): real per-call time varies far more
# within a module than the charged gas does (a group_ops pairing is ~18x a
# G1 add), and a per-byte gas rate can be disproportionate to the per-call
# rate relative to real time (ecvrf) — one gas column alone under-predicts
# exactly the expensive cases.
BASE_PREDICTORS = [
    "interp_instruction_count",
    "interp_stack_size_flow",
    "interp_stack_height_flow",
    "input_object_count",
    "input_object_bytes",
    "child_object_reads",
    "child_object_read_bytes",
    "packages_loaded",
    "package_bytes_loaded",
    "written_object_count",
    "written_bytes",
    "deleted_object_count",
    "event_count",
    "event_bytes",
]
INTERPRETER_COMPONENTS = BASE_PREDICTORS[:3]
COVERAGE_TARGET = 0.99
HOLDOUT_FRACTION = 0.2
VIF_THRESHOLD = 30.0


def load_dataset(root: Path):
    """Load a sweep.py dataset directory, or a single .jsonl capture such as
    the output of `iota-replay ch --profile-output` (its rows are labeled
    with the file stem instead of a sweep name)."""
    rows, manifest = [], None
    if root.is_file():
        run_files = [(root, root.stem)]
    else:
        mf = root / "manifest.json"
        if mf.exists():
            manifest = json.loads(mf.read_text())
        run_files = [(rf, rf.parts[len(root.parts)])
                     for rf in sorted(root.glob("*/*/run-*.jsonl"))]
    for rf, sweep in run_files:
        with open(rf) as f:
            for line in f:
                row = json.loads(line)
                if "meta" in row:
                    continue
                row["sweep"] = sweep
                rows.append(row)
    return rows, manifest


def build_matrix(rows):
    native_cols = sorted({
        f for r in rows for f in r["profile"].get("native_gas_by_function", {})
    })
    columns = (BASE_PREDICTORS
               + [f"native_gas[{f}]" for f in native_cols]
               + [f"native_calls[{f}]" for f in native_cols])
    xs, ys = [], []
    for r in rows:
        p = r["profile"]
        x = [float(p.get(c, 0)) for c in BASE_PREDICTORS]
        per_fn_gas = p.get("native_gas_by_function", {})
        per_fn_calls = p.get("native_calls_by_function", {})
        x += [float(per_fn_gas.get(f, 0)) for f in native_cols]
        x += [float(per_fn_calls.get(f, 0)) for f in native_cols]
        x.append(1.0)  # intercept, last column
        xs.append(x)
        ys.append(float(r["measured_ns"]))
    return columns + ["intercept"], xs, ys


def gram(xs, ys):
    k = len(xs[0])
    g = [[0.0] * k for _ in range(k)]
    gy = [0.0] * k
    for x, y in zip(xs, ys):
        for i in range(k):
            xi = x[i]
            if xi == 0.0:
                continue
            gy[i] += xi * y
            gi = g[i]
            for j in range(i, k):
                gi[j] += xi * x[j]
    for i in range(k):
        for j in range(i + 1, k):
            g[j][i] = g[i][j]
    return g, gy


def solve(a, b):
    """Gaussian elimination with partial pivoting; returns None if singular."""
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for col in range(n):
        piv = max(range(col, n), key=lambda r: abs(m[r][col]))
        if abs(m[piv][col]) < 1e-12:
            return None
        m[col], m[piv] = m[piv], m[col]
        for r in range(n):
            if r != col and m[r][col] != 0.0:
                f = m[r][col] / m[col][col]
                for c in range(col, n + 1):
                    m[r][c] -= f * m[col][c]
    return [m[i][n] / m[i][i] for i in range(n)]


def nnls(g, gy, k):
    """Lawson-Hanson active-set NNLS on the Gram matrix."""
    passive = []
    w = [0.0] * k
    for _ in range(3 * k):
        residual_grad = [gy[i] - sum(g[i][j] * w[j] for j in passive) for i in range(k)]
        candidates = [(residual_grad[i], i) for i in range(k)
                      if i not in passive and residual_grad[i] > 1e-9]
        if not candidates:
            break
        passive.append(max(candidates)[1])
        while True:
            sub_g = [[g[i][j] for j in passive] for i in passive]
            sub_y = [gy[i] for i in passive]
            sol = solve(sub_g, sub_y)
            if sol is None:
                # Singular subproblem: drop the newest column.
                passive.pop()
                break
            if all(s > 0 for s in sol):
                for j in passive:
                    w[j] = 0.0
                for idx, j in enumerate(passive):
                    w[j] = sol[idx]
                break
            # Move toward sol until the first coefficient hits zero, drop it.
            alpha, drop = 1.0, None
            for idx, j in enumerate(passive):
                if sol[idx] <= 0 and w[j] - sol[idx] > 0:
                    a = w[j] / (w[j] - sol[idx])
                    if a < alpha:
                        alpha, drop = a, j
            for idx, j in enumerate(passive):
                w[j] += alpha * (sol[idx] - w[j])
            if drop is not None:
                passive.remove(drop)
                w[drop] = 0.0
    return w


def vif(columns, xs, targets):
    """Variance inflation factor of each target within the target group.

    Each target is regressed on the *other targets* plus the intercept, not
    on all columns: the sweeps compose transactions from a small set of
    building blocks, so every counter is an exact linear function of the
    block counts and against-all-columns VIF is structurally infinite. The
    question the gate answers is whether the components can be told apart
    from each other. Residuals are recomputed explicitly per row (deriving
    them from Gram sums cancels catastrophically).
    """
    g, _ = gram(xs, [0.0] * len(xs))
    out = {}
    intercept = columns.index("intercept")
    for t in targets:
        ti = columns.index(t)
        others = [columns.index(o) for o in targets if o != t] + [intercept]
        sub_g = [[g[i][j] for j in others] for i in others]
        sub_y = [g[i][ti] for i in others]
        sol = solve(sub_g, sub_y)
        if sol is None:
            out[t] = float("inf")
            continue
        mean_t = sum(x[ti] for x in xs) / len(xs)
        ss_tot = sum((x[ti] - mean_t) ** 2 for x in xs)
        ss_res = sum(
            (x[ti] - sum(c * x[j] for c, j in zip(sol, others))) ** 2 for x in xs
        )
        out[t] = float("inf") if ss_res <= 0 else round(ss_tot / ss_res, 1)
    return out


def predict(w, x):
    return sum(wi * xi for wi, xi in zip(w, x))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", nargs="+", type=Path, required=True,
                    help="sweep.py dataset directories")
    ap.add_argument("--out", type=Path, default=Path("calibration-artifact.json"))
    ap.add_argument("--holdout-seed", type=int, default=7)
    args = ap.parse_args()

    rows, manifests = [], []
    for d in args.data:
        r, m = load_dataset(d)
        print(f"{d}: {len(r)} transactions")
        rows.extend(r)
        if m:
            manifests.append(m)
    if len(rows) < 100:
        sys.exit("too few rows to fit")

    columns, xs, ys = build_matrix(rows)

    # Fit in relative error: divide each row by its measured time, so the
    # loss matches the ratio-based acceptance criteria (coverage, p95
    # overestimate) and a 20 ms transaction does not outweigh a thousand
    # 25 µs ones the way absolute least squares would have it.
    xs_fit = [[xi / y for xi in x] for x, y in zip(xs, ys)]
    ys_fit = [1.0] * len(ys)

    # Column scaling for conditioning; coefficients are unscaled afterwards.
    scales = []
    for i in range(len(columns)):
        s = max(abs(x[i]) for x in xs_fit) or 1.0
        scales.append(s)
    xs_s = [[xi / s for xi, s in zip(x, scales)] for x in xs_fit]

    # Deterministic holdout split.
    import random
    rng = random.Random(args.holdout_seed)
    idx = list(range(len(rows)))
    rng.shuffle(idx)
    cut = int(len(idx) * (1 - HOLDOUT_FRACTION))
    train, hold = idx[:cut], idx[cut:]

    g, gy = gram([xs_s[i] for i in train], [ys_fit[i] for i in train])
    w_s = nnls(g, gy, len(columns))
    w = [wi / s for wi, s in zip(w_s, scales)]

    # Fit quality on the training split.
    preds = [predict(w, xs[i]) for i in train]
    actual = [ys[i] for i in train]
    mean_y = sum(actual) / len(actual)
    ss_res = sum((a - p) ** 2 for a, p in zip(actual, preds))
    ss_tot = sum((a - mean_y) ** 2 for a in actual)
    r2 = 1 - ss_res / ss_tot

    # Safety multiplier on the holdout: smallest m with coverage >= target.
    ratios = sorted(ys[i] / max(predict(w, xs[i]), 1.0) for i in hold)
    m_mult = ratios[min(int(COVERAGE_TARGET * len(ratios)), len(ratios) - 1)]
    over = sorted(max(predict(w, xs[i]), 1.0) * m_mult / ys[i] for i in hold)
    p95_over = over[int(0.95 * (len(over) - 1))]

    # Separability gate for the interpreter components.
    vifs = vif(columns, xs_s, INTERPRETER_COMPONENTS)
    separable = all(v < VIF_THRESHOLD for v in vifs.values())

    # When the gate fails, the plan prescribes shipping one combined
    # coefficient for the entangled components: refit with their columns
    # summed into one, so the artifact carries the shippable constant.
    combined_fit = None
    entangled = sorted(t for t, v in vifs.items() if v >= VIF_THRESHOLD)
    if not separable and len(entangled) >= 2:
        keep = entangled[0]
        drop = set(entangled[1:])
        merged_columns = [c for c in columns if c not in drop]
        keep_i = columns.index(keep)
        drop_i = [columns.index(c) for c in drop]
        xs_m = []
        for x in xs:
            row = [
                (x[keep_i] + sum(x[j] for j in drop_i)) if i == keep_i else v
                for i, v in enumerate(x)
                if columns[i] not in drop
            ]
            xs_m.append(row)
        xs_m_fit = [[xi / y for xi in x] for x, y in zip(xs_m, ys)]
        scales_m = [max(abs(x[i]) for x in xs_m_fit) or 1.0 for i in range(len(merged_columns))]
        xs_ms = [[xi / sc for xi, sc in zip(x, scales_m)] for x in xs_m_fit]
        g_m, gy_m = gram([xs_ms[i] for i in train], [1.0 for _ in train])
        w_ms = nnls(g_m, gy_m, len(merged_columns))
        w_m = [wi / sc for wi, sc in zip(w_ms, scales_m)]
        ratios_m = sorted(
            ys[i] / max(predict(w_m, xs_m[i]), 1.0) for i in hold
        )
        m_mult_m = ratios_m[min(int(COVERAGE_TARGET * len(ratios_m)), len(ratios_m) - 1)]
        combined_fit = {
            "merged_into": f"{' + '.join(entangled)}",
            "coefficients_ns_per_unit": {
                c: w_m[i]
                for i, c in enumerate(merged_columns)
                if c != "intercept" and w_m[i] > 0
            },
            "c0_ns": w_m[merged_columns.index("intercept")],
            "safety_multiplier": m_mult_m,
        }

    # Anchor comparison: each sweep's single-knob slope vs. the fitted
    # coefficient for the same counter (a large gap means covarying work,
    # not necessarily an error — the anchors are comparisons, not truth).
    anchors = {}
    for d in args.data:
        sf = d / "slopes.json"
        if sf.exists():
            for sweep, slope in json.loads(sf.read_text()).items():
                xf = slope["x_field"]
                if xf in columns:
                    anchors[sweep] = {
                        "x_field": xf,
                        "anchor_ns_per_unit": slope["ns_per_unit"],
                        "fitted_ns_per_unit": w[columns.index(xf)],
                    }

    # Per-sweep residuals: which shapes the model mispredicts.
    by_sweep = {}
    for i in hold:
        by_sweep.setdefault(rows[i]["sweep"], []).append(
            predict(w, xs[i]) * m_mult / ys[i]
        )
    sweep_ratios = {
        s: round(statistics.median(v), 3) for s, v in sorted(by_sweep.items())
    }

    artifact = {
        "model": "measured_ns = m * (c0 + sum_j w_j * count_j)",
        "unix_time_secs": int(time.time()),
        "coefficients_ns_per_unit": {
            c: w[i] for i, c in enumerate(columns) if c != "intercept" and w[i] > 0
        },
        "c0_ns": w[columns.index("intercept")],
        "safety_multiplier": m_mult,
        "coverage_target": COVERAGE_TARGET,
        "holdout_p95_overestimate": p95_over,
        "train_r_squared": r2,
        "interpreter_separability": {
            "vif": vifs,
            "threshold": VIF_THRESHOLD,
            "separable": separable,
        },
        "combined_interpreter_fit": combined_fit,
        "anchors": anchors,
        "holdout_median_overestimate_by_sweep": sweep_ratios,
        "n_transactions": len(rows),
        "datasets": [str(d) for d in args.data],
        "manifests": manifests,
    }
    args.out.write_text(json.dumps(artifact, indent=2) + "\n")

    print(f"\nfit over {len(train)} train / {len(hold)} holdout rows, R²={r2:.4f}")
    print(f"c0 = {w[columns.index('intercept')] / 1000:.1f} µs")
    for i, c in enumerate(columns):
        if c != "intercept" and w[i] > 0:
            print(f"  {c}: {w[i]:.3f} ns/unit")
    print(f"safety multiplier m = {m_mult:.3f} "
          f"(p95 overestimate x{p95_over:.2f} at {COVERAGE_TARGET:.0%} coverage)")
    print(f"interpreter separability: {vifs} -> "
          f"{'separable' if separable else 'NOT separable: ship one combined coefficient'}")
    if combined_fit:
        combined = combined_fit["merged_into"]
        value = combined_fit["coefficients_ns_per_unit"].get(
            entangled[0], 0.0
        )
        print(f"combined-coefficient fit: [{combined}] = {value:.3f} ns/unit "
              f"(m = {combined_fit['safety_multiplier']:.3f}) — the shippable set")
    print(f"artifact written to {args.out}")


if __name__ == "__main__":
    main()
