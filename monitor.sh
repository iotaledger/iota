#!/usr/bin/env bash
# monitor.sh — watch progress of a running run.sh sweep.
#
# Modes:
#   ./monitor.sh          (default) live-tail of meaningful event lines
#                         from both run.log and sweep.log. Ctrl-C to stop.
#   ./monitor.sh status   one-shot snapshot: running processes,
#                         per-policy iter count, last log lines.
#   ./monitor.sh raw      live-tail of ALL output (unfiltered, noisy).

set -uo pipefail
cd "$(dirname "$0")"

# Detect TTY once at the top. Must be checked here (not in $() subshells,
# where stdout is a pipe and the check would always read as non-TTY).
if [ -t 1 ]; then IS_TTY=1; else IS_TTY=0; fi
export IS_TTY

# Colors (auto-disable when stdout is not a TTY).
if [ "$IS_TTY" = "1" ]; then
  C_BOLD=$'\033[1m';  C_DIM=$'\033[2m'
  C_RED=$'\033[31m';  C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
  C_MAGENTA=$'\033[35m'; C_CYAN=$'\033[36m'
  C_RESET=$'\033[0m'
else
  C_BOLD=""; C_DIM=""; C_RED=""; C_GREEN=""
  C_YELLOW=""; C_BLUE=""; C_MAGENTA=""; C_CYAN=""; C_RESET=""
fi
section() { echo; echo "${C_BOLD}${C_CYAN}=== $* ===${C_RESET}"; }
info()    { echo "${C_DIM}$*${C_RESET}"; }
warn()    { echo "${C_YELLOW}$*${C_RESET}"; }
good()    { echo "${C_GREEN}$*${C_RESET}"; }
bad()     { echo "${C_RED}$*${C_RESET}"; }

# sed colorizer for the live-tail stream. Each rule highlights one line
# kind. Use sed's `\033` escapes; mixing $'…' into a single sed -e is fragile,
# so we just build the escape sequences inline.
colorize_stream() {
  if [ "$IS_TTY" != "1" ]; then
    # No TTY → no colors, just pass through.
    cat
    return
  fi
  sed -u \
    -e $'s/^=== run\\.sh.*$/\e[1;36m&\e[0m/' \
    -e $'s/^=== run_inner.*$/\e[1;36m&\e[0m/' \
    -e $'s/^=== FAST_MODE.*$/\e[1;35m&\e[0m/' \
    -e $'s/.*full network reset.*/\e[1;31m&\e[0m/' \
    -e $'s/.*exited non-zero.*/\e[31m&\e[0m/' \
    -e $'s/.*FAILED.*/\e[31m&\e[0m/' \
    -e $'s/.*fail_streak.*/\e[33m&\e[0m/' \
    -e $'s/.*validators ready after.*/\e[32m&\e[0m/' \
    -e $'s/.*drained after.*/\e[2m&\e[0m/' \
    -e $'s/^\\[sweep iter=.*$/\e[1;34m&\e[0m/' \
    -e $'s/^>>> RESULT.*/\e[1;32m&\e[0m/'
}

MODE="${1:-tail}"

case "$MODE" in
  tail|"")
    section "live monitor (Ctrl-C to stop)"
    info "  Logs: run.log + sweep.log"
    echo

    # Colorize one line in-place (bash function — no pipe overhead).
    # The case patterns mirror the grep filter below.
    colorize_line() {
      local line="$1"
      case "$line" in
        "=== run.sh"*|"=== run_inner"*)   printf '%s\n' "${C_BOLD}${C_CYAN}${line}${C_RESET}" ;;
        "=== FAST_MODE"*)                 printf '%s\n' "${C_BOLD}${C_MAGENTA}${line}${C_RESET}" ;;
        *"full network reset"*)           printf '%s\n' "${C_BOLD}${C_RED}${line}${C_RESET}" ;;
        *"exited non-zero"*|*"FAILED"*)   printf '%s\n' "${C_RED}${line}${C_RESET}" ;;
        *"fail_streak"*)                  printf '%s\n' "${C_YELLOW}${line}${C_RESET}" ;;
        *"validators ready after"*)       printf '%s\n' "${C_GREEN}${line}${C_RESET}" ;;
        *"drained after"*)                printf '%s\n' "${C_DIM}${line}${C_RESET}" ;;
        "[sweep iter"*)                   printf '%s\n' "${C_BOLD}${C_BLUE}${line}${C_RESET}" ;;
        ">>> RESULT"*)                    printf '%s\n' "${C_BOLD}${C_GREEN}${line}${C_RESET}" ;;
        *)                                printf '%s\n' "$line" ;;
      esac
    }

    # Read the latest sweep.jsonl record and print a compact 5-metric
    # summary line: peak (cap discipline) + tps + B_p99 + e2e_p99 +
    # honest_cl fairness. Plus elapsed wall-clock for the iter.
    print_iter_summary() {
      local elapsed="$1"
      # Give JSONL a moment to flush after >>> RESULT prints.
      sleep 0.3
      ELAPSED="$elapsed" python3 <<'PY'
import json, os, sys
is_tty = os.environ.get("IS_TTY") == "1"
def c(code, s): return f"\033[{code}m{s}\033[0m" if is_tty else s
def bold(s):  return c("1", s)
def dim(s):   return c("2", s)
def red(s):   return c("31", s)
def green(s): return c("32", s)
def yellow(s):return c("33", s)
def cyan(s):  return c("36", s)
try:
    with open("sweep.jsonl") as f:
        lines = f.readlines()
    if not lines:
        raise SystemExit
    r = json.loads(lines[-1])
    if r.get("failed"):
        # Failed iter — nothing meaningful to summarise.
        raise SystemExit
    res = r["results"]
    v = r["validator"]

    peak = res.get("peak_inflight", 0) or 0
    maxp = v.get("max_pending_transactions", 1000) or 1000
    over = peak - maxp
    over_str = f"+{over}" if over >= 0 else str(over)
    over_color = red if over > 0 else green

    tps   = res.get("useful_tps", 0) or 0
    b_p99 = res.get("permit_wait_p99", 0) or 0
    e2e   = res.get("consensus_lat_p99", 0) or 0

    # RED-fairness ratio (Floyd & Jacobson 1993 Claim 2: uniform drop
    # probability across sources). Computed as:
    #   honest_admit_fraction / spammer_admit_fraction
    # honest = honest_cl pool (closed-loop, has reliable bench_success);
    # spammer admit = (useful_tps * spam_dur - honest_cl_commits) / spam_offered.
    # 1.0 = uniform (RED-ideal); >>1 = honest gets timing-luck advantage
    # under binary tail-drop; should be closer to 1 under graduated.
    iw = r.get("iter_window") or {}
    spam_dur = (iw.get("spam_end_epoch") or 0) - (iw.get("spam_start_epoch") or 0)
    hcl = r.get("honest_cl") or {}
    hcl_commits = hcl.get("bench_success") or 0
    hcl_offered = hcl.get("offered") or 0
    spam_offered = (r.get("spammer") or {}).get("offered") or 0
    total_commits = tps * spam_dur
    spam_commits = max(total_commits - hcl_commits, 0)
    hcl_admit_frac = hcl_commits / hcl_offered if hcl_offered else 0
    spam_admit_frac = spam_commits / spam_offered if spam_offered else 0
    red_ratio = hcl_admit_frac / spam_admit_frac if spam_admit_frac else 0

    # Color the ratio by closeness to 1.0 (uniform):
    #   ≤2: green (close to uniform)
    #   2-5: yellow (moderate bias)
    #   >5: red (strong bias / phase-effect)
    if red_ratio == 0:
        ratio_str = dim("n/a")
    elif red_ratio <= 2:
        ratio_str = green(f"{red_ratio:.1f}")
    elif red_ratio <= 5:
        ratio_str = yellow(f"{red_ratio:.1f}")
    else:
        ratio_str = red(f"{red_ratio:.1f}")

    elapsed = os.environ.get("ELAPSED", "?")
    parts = [
        f"  {dim('↳')} {dim(f'{elapsed}s wall')}",
        f"peak={bold(peak)}{over_color(f'({over_str})')}",
        f"tps={bold(f'{tps:.0f}')}",
        f"B_p99={bold(f'{b_p99:.2f}s')}",
        f"e2e_p99={bold(f'{e2e:.2f}s')}",
        f"RED_ratio={ratio_str}",
    ]
    print("  ".join(parts))
except (FileNotFoundError, IndexError, json.JSONDecodeError, KeyError):
    pass
PY
    }

    iter_start=0
    # -F: follow by name (survives log rotation / fresh creation)
    # 2>/dev/null: don't complain if a log file doesn't exist yet
    tail -F sweep.log run.log 2>/dev/null \
      | grep --line-buffered -E '^=== run\.sh|^=== run_inner|^\[sweep iter|^>>> RESULT|^=== FAST_MODE|fail_streak|validators ready after|exited non-zero|full network reset|drained after|FAILED' \
      | while IFS= read -r line; do
          # Track iter start time for elapsed-wall reporting.
          case "$line" in
            "[sweep iter"*) iter_start=$(date +%s) ;;
          esac
          colorize_line "$line"
          # After each iter completes, print the compact summary + blank
          # line so the next iter's output starts visually separated.
          if [[ "$line" == ">>> RESULT"* ]]; then
            elapsed=0
            if [ "${iter_start:-0}" -gt 0 ]; then
              elapsed=$(( $(date +%s) - iter_start ))
            fi
            print_iter_summary "$elapsed"
            echo
          fi
        done
    ;;

  raw)
    section "live monitor (RAW, Ctrl-C to stop)"
    tail -F sweep.log run.log 2>/dev/null | colorize_stream
    ;;

  status|s)
    section "process check"
    out=$(pgrep -af "run_inner.sh|sweep.sh|stress-multi.sh|target/release/stress " 2>/dev/null)
    if [ -z "$out" ]; then
      good "  (no sweep processes running)"
    else
      echo "$out" | sed 's/^/  /'
    fi

    section "record count"
    if [ -f sweep.jsonl ]; then
      total=$(wc -l < sweep.jsonl)
      info "  sweep.jsonl: ${C_BOLD}$total${C_RESET}${C_DIM} records${C_RESET}"
      python3 <<'PY'
import json, os, sys
from collections import Counter
is_tty = os.environ.get("IS_TTY") == "1"
def red(s): return f"\033[31m{s}\033[0m" if is_tty else s
def green(s): return f"\033[32m{s}\033[0m" if is_tty else s
try:
    with open("sweep.jsonl") as f:
        recs = [json.loads(l) for l in f if l.strip()]
    if not recs:
        print("  (empty)")
        raise SystemExit
    c = Counter()
    failed = 0
    for r in recs:
        if r.get("failed"):
            failed += 1
        v = r["validator"]
        k = (
            v["max_pending_transactions"],
            v["graduated_load_shedding_soft_limit_pct"],
            v.get("graduated_load_shedding_saturation_pct", 100),
        )
        c[k] += 1
    fail_str = (red if failed else green)(f"{failed} failed")
    print(f"  per-policy iter count ({fail_str}):")
    for k, n in sorted(c.items()):
        print(f"    max={k[0]:>5}  pct={k[1]:>3}  sat={k[2]:>3}  n={n}")
except FileNotFoundError:
    pass
except Exception as e:
    print(f"  error reading jsonl: {e}", file=sys.stderr)
PY
    else
      warn "  sweep.jsonl: missing"
    fi

    section "last 5 lines of sweep.log"
    if [ -f sweep.log ]; then
      tail -5 sweep.log | colorize_stream | sed 's/^/  /'
    else
      warn "  (no sweep.log)"
    fi

    section "last 5 lines of run.log"
    if [ -f run.log ]; then
      tail -5 run.log | colorize_stream | sed 's/^/  /'
    else
      warn "  (no run.log)"
    fi
    ;;

  *)
    bad "Usage: $0 [tail|status|raw]"
    echo "  tail   (default) filtered live progress"
    echo "  status one-shot snapshot of state"
    echo "  raw    unfiltered live tail (very noisy)"
    exit 1
    ;;
esac
