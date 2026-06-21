"""Parsers for IOTA validator, fullnode, and stress-benchmark logs.

Each parser is a generator yielding typed event namedtuples. Lines are pre-filtered
by cheap substring tests before regex application to keep the cost flat on multi-
gigabyte logs.
"""

from __future__ import annotations

import re
from collections import namedtuple
from typing import Iterator, Optional

# ---------- Event types ---------------------------------------------------

# Validator events
WinnerLockAcquired = namedtuple(
    "WinnerLockAcquired", "validator digest num_inputs ts"
)
# A transaction dropped post-consensus because one of its owned inputs was
# already locked by an earlier transaction. The node resolves all three lock
# tiers (same-commit / consensus-quarantine / persistent DB) through a single
# `find_existing_lock` path and emits one unified line, so the audit models the
# loser with a single event type. `locked_by` is the digest of the winning tx
# that holds the lock on `(obj_id, obj_version)`.
LoserDropped = namedtuple(
    "LoserDropped",
    "validator digest obj_id obj_version obj_digest locked_by ts",
)
BatchSummary = namedtuple(
    "BatchSummary", "validator num_dropped num_retained ts"
)
Executed = namedtuple("Executed", "validator digest fx_digest ts")

# Fullnode events
FnSubmissionSeen = namedtuple("FnSubmissionSeen", "digest ts")
FnFinalFailure = namedtuple("FnFinalFailure", "digest reason ts")
FnEffectsExecuted = namedtuple(
    "FnEffectsExecuted", "digest effects_digest validator ts"
)

# Stress events
StressAttempt = namedtuple("StressAttempt", "digest retry_cnt err ts")
StressGaveUp = namedtuple("StressGaveUp", "digest attempts ts")
StressExpectedFailure = namedtuple("StressExpectedFailure", "ts")
# Emitted by the double-spend workload right before each tx is submitted.
# `pair_id` identifies the conflict group: all attempts with the same pair_id
# contest the same gas coin and are expected to collide.
DoubleSpendAttempt = namedtuple(
    "DoubleSpendAttempt", "pair_id gas_object gas_version digest sink ts"
)


# ---------- Common patterns -----------------------------------------------

# Each line starts with the app's RFC3339 timestamp as the first token.
_TS_RE = re.compile(r"^(\S+)\s+")

# Validator-side regexes
_RE_WINNER = re.compile(
    r'Transaction passed post-consensus validation, acquired all object locks '
    r'digest=Digest\("([^"]+)"\) num_owned_inputs=(\d+)'
)
_RE_LOSER = re.compile(
    r'Transaction conflicts with existing owned-object lock, dropping '
    r'digest=Digest\("([^"]+)"\) obj_ref=ObjectReference \{ '
    r'object_id: ObjectId\("([^"]+)"\), version: Version\((\d+)\), '
    r'digest: Digest\("([^"]+)"\) \} locked_by=Digest\("([^"]+)"\)'
)
_RE_BATCH = re.compile(
    r'Post-consensus validation dropped transactions '
    r'num_dropped=(\d+) num_retained=(\d+)'
)
_RE_EXEC = re.compile(
    r'process_transaction succeeded tx_digest=Digest\("([^"]+)"\) '
    r'fx_digest=Digest\("([^"]+)"\)'
)

# Fullnode-side regexes
_RE_FN_SPAN_DIGEST = re.compile(
    r'drive_transaction\{tx_digest=Some\(Digest\("([^"]+)"\)\)'
)
_RE_FN_FINAL_FAIL = re.compile(
    r'User transaction (?:failed to finalize|timed out) .*?: (.+?)$'
)
_RE_FN_EXEC_RETURN = re.compile(
    r'effects_certifier: return=\[\(Digest\("([^"]+)"\), '
    r'Executed \{ effects_digest: Digest\("([^"]+)"\)'
)
_RE_FN_VALIDATOR = re.compile(r'validator_display_name="([^"]+)"')

# Stress-side regexes
_RE_STRESS_RETRY = re.compile(
    r'Transaction failed with err: (.*?) '
    r'tx_digest=Digest\("([^"]+)"\) retry_cnt=(\d+)'
)
_RE_STRESS_GAVE_UP = re.compile(
    r'Transaction execution got error: Transaction Digest\("([^"]+)"\) '
    r'failed for (\d+) times'
)
_RE_DOUBLE_SPEND_SUBMIT = re.compile(
    r'pair_id=(\d+) '
    r'gas_object=ObjectId\("([^"]+)"\) '
    r'gas_version=Version\((\d+)\) '
    r'tx_digest=Digest\("([^"]+)"\)'
    r'(?: sink=(\S+))?'
)


# ---------- Helpers -------------------------------------------------------

def _app_ts(line: str) -> Optional[str]:
    m = _TS_RE.match(line)
    return m.group(1) if m else None


def _app_ts_fast(line: str) -> Optional[str]:
    """Pure-string variant of _app_ts — avoids regex overhead in hot paths.
    Returns the first whitespace-separated token (the app's RFC3339 timestamp).
    Logs collected via `docker logs` without `-t` begin directly with the app
    timestamp; with `-t` the leading docker timestamp is itself valid, so the
    first token is correct either way."""
    space1 = line.find(" ")
    if space1 == -1:
        return None
    return line[:space1]


# ---------- Validator parser ----------------------------------------------

def parse_validator_log(
    path: str,
    validator: str,
    progress_cb=None,
    progress_every: int = 500_000,
) -> Iterator[tuple]:
    with open(path, "r", errors="replace") as f:
        for line_no, line in enumerate(f, 1):
            if progress_cb is not None and line_no % progress_every == 0:
                progress_cb(line_no)

            # Cheap pre-filter — vast majority of lines drop out here.
            if (
                "post_consensus_validation" not in line
                and "process_transaction succeeded" not in line
            ):
                continue

            ts = _app_ts_fast(line)
            if ts is None:
                continue

            if "passed post-consensus validation" in line:
                m = _RE_WINNER.search(line)
                if m:
                    yield WinnerLockAcquired(
                        validator, m.group(1), int(m.group(2)), ts
                    )
                continue

            if "conflicts with existing owned-object lock" in line:
                m = _RE_LOSER.search(line)
                if m:
                    yield LoserDropped(
                        validator,
                        m.group(1),
                        m.group(2),
                        int(m.group(3)),
                        m.group(4),
                        m.group(5),
                        ts,
                    )
                continue

            if "Post-consensus validation dropped" in line:
                m = _RE_BATCH.search(line)
                if m:
                    yield BatchSummary(
                        validator, int(m.group(1)), int(m.group(2)), ts
                    )
                continue

            if "process_transaction succeeded" in line:
                m = _RE_EXEC.search(line)
                if m:
                    yield Executed(validator, m.group(1), m.group(2), ts)
                continue


# ---------- Fullnode parser -----------------------------------------------

_FN_DIGEST_PREFIX = 'drive_transaction{tx_digest=Some(Digest("'
_FN_DIGEST_PREFIX_LEN = len(_FN_DIGEST_PREFIX)


def parse_fullnode_log(
    path: str,
    progress_cb=None,
    progress_every: int = 250_000,
    max_lines: int = 0,
) -> Iterator[tuple]:
    """Yields submission-related events. Most lines are repeated span context;
    we only emit:
      - FnSubmissionSeen on first occurrence of a digest in any drive_transaction span
      - FnFinalFailure   on terminal "User transaction failed/timed out" lines
      - FnEffectsExecuted on effects_certifier return=[(Digest, Executed {...})]

    Hot path uses pure string ops (find/substring) rather than regex; regex
    only fires on the rare lines that match a terminal-event substring.

    Assumption: every line of interest carries the `drive_transaction{tx_digest=
    Some(Digest("..."))}` span — the digest is extracted from it first and the
    line is skipped outright if absent. This holds because the terminal-failure
    and effects-return logs are emitted inside that span, but it means a failure
    or execution line that ever loses the span would be silently dropped (the
    coverage check guards only against *zero* fullnode submissions, not a
    partial miss).

    progress_cb(line_no, n_submissions, n_failures, n_executed) is called every
    `progress_every` lines for monitoring long runs.
    """
    seen_digests: set = set()
    n_sub = n_fail = n_exec = 0
    line_no = 0  # bound for the final progress tick even if the file is empty

    with open(path, "r", errors="replace") as f:
        for line_no, line in enumerate(f, 1):
            if progress_cb is not None and line_no % progress_every == 0:
                progress_cb(line_no, n_sub, n_fail, n_exec)
            if max_lines and line_no >= max_lines:
                break

            # Locate the digest substring directly without regex.
            pos = line.find(_FN_DIGEST_PREFIX)
            if pos == -1:
                continue
            start = pos + _FN_DIGEST_PREFIX_LEN
            end = line.find('"', start)
            if end == -1:
                continue
            digest = line[start:end]

            ts = _app_ts_fast(line)
            if ts is None:
                continue

            # First-sight tracking.
            if digest not in seen_digests:
                seen_digests.add(digest)
                n_sub += 1
                yield FnSubmissionSeen(digest, ts)

            # Terminal failure — only the top-level drive_transaction INFO log.
            # Cheap substring gate first; regex only on candidates.
            # "Retrying ..." marks the retriable per-attempt log (not terminal):
            # the tx may still go on to win, so counting it as a final failure
            # produces spurious cross-validator disagreement (see Check E).
            if "User transaction" in line and (
                "failed to finalize" in line or "timed out" in line
            ):
                if (
                    "submit_transaction" not in line
                    and "drive_transaction_once" not in line
                    and "Retrying" not in line
                ):
                    fm = _RE_FN_FINAL_FAIL.search(line)
                    reason = fm.group(1).strip() if fm else line.strip()
                    n_fail += 1
                    yield FnFinalFailure(digest, reason, ts)
                continue

            # Execution effects returned from a validator.
            if "effects_certifier: return=" in line and "Executed {" in line:
                em = _RE_FN_EXEC_RETURN.search(line)
                if em:
                    vm = _RE_FN_VALIDATOR.search(line)
                    n_exec += 1
                    yield FnEffectsExecuted(
                        em.group(1),
                        em.group(2),
                        vm.group(1) if vm else "?",
                        ts,
                    )

    # Final progress tick.
    if progress_cb is not None:
        progress_cb(line_no, n_sub, n_fail, n_exec)


# ---------- Stress parser -------------------------------------------------

def parse_stress_log(
    path: str,
    progress_cb=None,
    progress_every: int = 500_000,
) -> Iterator[tuple]:
    with open(path, "r", errors="replace") as f:
        for line_no, line in enumerate(f, 1):
            if progress_cb is not None and line_no % progress_every == 0:
                progress_cb(line_no)

            # Lines from the double-spend workload use target=double_spend,
            # so the `iota_benchmark` substring is absent. Accept either.
            if "iota_benchmark" not in line and "double_spend" not in line:
                continue
            ts = _app_ts_fast(line)
            if ts is None:
                continue

            if "Transaction failed with err:" in line:
                m = _RE_STRESS_RETRY.search(line)
                if m:
                    yield StressAttempt(
                        m.group(2), int(m.group(3)), m.group(1), ts
                    )
                continue

            if "Transaction execution got error:" in line:
                m = _RE_STRESS_GAVE_UP.search(line)
                if m:
                    yield StressGaveUp(m.group(1), int(m.group(2)), ts)
                continue

            if "Transaction failed with expected failure type" in line:
                yield StressExpectedFailure(ts)
                continue

            if "submitting double-spend tx" in line:
                m = _RE_DOUBLE_SPEND_SUBMIT.search(line)
                if m:
                    yield DoubleSpendAttempt(
                        int(m.group(1)),
                        m.group(2),
                        int(m.group(3)),
                        m.group(4),
                        m.group(5) or "",
                        ts,
                    )
                continue
