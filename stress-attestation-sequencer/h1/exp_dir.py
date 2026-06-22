#!/usr/bin/env python3
"""exp_dir.py — config-gated experiment directory for run.sh.

Given EXP_DIR (results/<LABEL>) and this run's config in CFG_* env vars, enforce
that every iteration under one LABEL shares the SAME config:

  - First run for a label: create EXP_DIR, write config.json (the contract).
  - Later run, config MATCHES: append a new iteration.
  - Later run, config DIFFERS: print a diff to stderr and exit non-zero (the run
    aborts) — so a label's pool can never mix configs. This replaces the old
    archive/ de-mixing hack.

On success prints the iteration dir name (iter-NNN) to stdout; run.sh uses it as
RESULTS_DIR = EXP_DIR/<iter-NNN>.
"""

import glob
import json
import os
import sys

EXP_DIR = sys.argv[1]
config = {k[4:]: v for k, v in os.environ.items() if k.startswith("CFG_")}
config_path = os.path.join(EXP_DIR, "config.json")


def die(msg, code=2):
    print(msg, file=sys.stderr)
    sys.exit(code)


if os.path.exists(config_path):
    try:
        existing = json.load(open(config_path))
    except Exception as e:  # noqa: BLE001
        die(f"ERROR: cannot read {config_path}: {e}")
    if existing != config:
        keys = sorted(set(existing) | set(config))
        diff = [
            f"  {k}: stored={existing.get(k, '<none>')!r}  now={config.get(k, '<none>')!r}"
            for k in keys
            if existing.get(k) != config.get(k)
        ]
        die(
            "ERROR: config mismatch for this LABEL — refusing to mix configs in one\n"
            f"experiment pool.\n  config file: {config_path}\n"
            + "\n".join(diff)
            + "\n\nUse a NEW LABEL for a different config, or delete the dir to reset:\n"
            f"  rm -rf {EXP_DIR}"
        )
else:
    os.makedirs(EXP_DIR, exist_ok=True)
    with open(config_path, "w") as f:
        json.dump(config, f, indent=2, sort_keys=True)
        f.write("\n")

# Next iteration index: highest existing iter-NNN + 1.
existing_iters = [
    int(os.path.basename(d).split("-")[1])
    for d in glob.glob(os.path.join(EXP_DIR, "iter-*"))
    if os.path.isdir(d) and os.path.basename(d).split("-")[1].isdigit()
]
nxt = (max(existing_iters) + 1) if existing_iters else 1
print(f"iter-{nxt:03d}")
