"""Invariant checks for double-spend safety verification.

Each check returns a CheckResult listing anomalies (FAIL or WARN). The overall
audit passes iff no check produces a FAIL.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from typing import Iterable, List

from parsers import (
    BatchSummary,
    DoubleSpendAttempt,
    Executed,
    FnEffectsExecuted,
    FnFinalFailure,
    FnSubmissionSeen,
    LoserDropped,
    StressGaveUp,
    WinnerLockAcquired,
)


@dataclass
class Anomaly:
    severity: str  # "FAIL" or "WARN"
    message: str
    evidence: dict = field(default_factory=dict)


@dataclass
class CheckResult:
    name: str
    description: str
    items_checked: int
    anomalies: List[Anomaly] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return not any(a.severity == "FAIL" for a in self.anomalies)

    @property
    def warned(self) -> bool:
        return any(a.severity == "WARN" for a in self.anomalies)


# Substrings that identify a transaction rejected BEFORE consensus (at
# submission), so it never reached the post-consensus owned-object contest and
# legitimately produces no winner/loser line. Surfaced client-side in the
# stress log's error and node-side in the fullnode's terminal-failure reason.
_PRE_CONSENSUS_REJECTION_MARKERS = (
    "rejected as invalid by more than 1/3 of validator stake during submission",
    "is not available for consumption, current version",
)


def is_pre_consensus_rejection(reason: str) -> bool:
    """True if `reason` is a pre-consensus (submission-time) rejection — a tx
    that never entered a consensus commit, so its absence from validator
    post-consensus logs is expected, not a completeness gap."""
    if not reason:
        return False
    return any(m in reason for m in _PRE_CONSENSUS_REJECTION_MARKERS)


# ---------- Coverage: the parser actually extracted the safety signal ------

def check_coverage(
    events: Iterable,
    num_validator_logs: int,
    num_stress_logs: int,
    num_double_spend_attempts: int,
    fullnode_enabled: bool,
    num_fn_submissions: int,
) -> CheckResult:
    """Guard against a vacuous PASS. Every other check derives its FAILs from
    the *presence* of loser / Executed events; if the parser matched none of
    them (e.g. the node's log format drifted from the regexes), those checks
    pass over empty input and the audit would otherwise certify "no
    double-spend leaked" while having verified nothing.

    This check FAILs when a signal that MUST be present for the double-spend
    workload is empty, so an empty/stale parse becomes a loud INCONCLUSIVE
    instead of a silent PASS. A FAIL here means "cannot certify", which the
    caller maps to a distinct exit code from a genuine safety violation.
    """
    n_winners = n_losers = n_executed = 0
    for ev in events:
        if isinstance(ev, WinnerLockAcquired):
            n_winners += 1
        elif isinstance(ev, LoserDropped):
            n_losers += 1
        elif isinstance(ev, Executed):
            n_executed += 1

    anomalies: List[Anomaly] = []
    if num_validator_logs > 0:
        for label, count in (
            ("winner (passed post-consensus validation)", n_winners),
            ("loser (conflicts with existing owned-object lock)", n_losers),
            ("executed (process_transaction succeeded)", n_executed),
        ):
            if count == 0:
                anomalies.append(
                    Anomaly(
                        severity="FAIL",
                        message=(
                            f"parsed 0 '{label}' events from "
                            f"{num_validator_logs} validator log(s) — the node "
                            f"log format may have changed or the workload "
                            f"exercised no conflicts; cannot certify safety"
                        ),
                        evidence={"event": label, "count": count},
                    )
                )

    if num_stress_logs > 0 and num_double_spend_attempts == 0:
        anomalies.append(
            Anomaly(
                severity="FAIL",
                message=(
                    f"parsed 0 double-spend submission events from "
                    f"{num_stress_logs} stress log(s) — cannot certify the "
                    f"workload actually contested any gas coins"
                ),
                evidence={"double_spend_attempts": 0},
            )
        )

    if fullnode_enabled and num_fn_submissions == 0:
        anomalies.append(
            Anomaly(
                severity="WARN",
                message=(
                    "fullnode parsing enabled but 0 submissions observed — "
                    "fullnode checks (E) ran on empty input (is the fullnode "
                    "RUST_LOG missing iota_core=debug?)"
                ),
                evidence={"fn_submissions": 0},
            )
        )

    return CheckResult(
        name="0",
        description="Parser coverage (signal present)",
        items_checked=n_winners + n_losers + n_executed,
        anomalies=anomalies,
    )


# ---------- A: Per-input single winner ------------------------------------

def check_single_winner_per_input(events: Iterable) -> CheckResult:
    """For each (object_id, version) input, at most one transaction may be
    declared the winner across all validators. More than one means different
    transactions were accepted for the same owned-object version — a true
    double-spend safety violation.

    Two independent signals feed the per-input winner set:
      - Winner evidence: a winner logs the object refs it acquired locks on, so
        two distinct winner digests naming the same (object_id, version) is a
        direct two-winners / no-loser leak.
      - Loser evidence: a loser names the tx that out-locked it (`locked_by`),
        so two distinct `locked_by` for the same input is also a leak.

    Both must agree (a fixed object version has exactly one legitimate winner),
    so they share one set per input and either source can surface the FAIL.
    Winner evidence closes the gap that loser evidence alone cannot see — a
    leak that produces two winners and no loser. It degrades gracefully: winner
    lines from a node build that logs only the input count contribute no input
    keys, leaving the loser-based check intact.
    """
    input_to_winners: dict = defaultdict(set)
    input_validators: dict = defaultdict(set)
    input_losers: dict = defaultdict(set)

    for ev in events:
        if isinstance(ev, WinnerLockAcquired):
            for key in ev.inputs:
                input_to_winners[key].add(ev.digest)
                input_validators[key].add(ev.validator)
        elif isinstance(ev, LoserDropped):
            key = (ev.obj_id, ev.obj_version)
            input_to_winners[key].add(ev.locked_by)
            input_validators[key].add(ev.validator)
            input_losers[key].add(ev.digest)

    anomalies: List[Anomaly] = []
    for inp, winners in input_to_winners.items():
        if len(winners) > 1:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=(
                        f"Input {inp[0]}@v{inp[1]} has {len(winners)} "
                        f"distinct declared winners — double-spend leaked"
                    ),
                    evidence={
                        "object_id": inp[0],
                        "version": inp[1],
                        "winners": sorted(winners),
                        "losers": sorted(input_losers[inp]),
                        "validators_reporting": sorted(input_validators[inp]),
                    },
                )
            )

    return CheckResult(
        name="A",
        description="Per-input single winner",
        items_checked=len(input_to_winners),
        anomalies=anomalies,
    )


# ---------- B: Cross-validator agreement ----------------------------------

def check_cross_validator_agreement(events: Iterable) -> CheckResult:
    """For each tx digest, every validator that observed it must reach the
    same verdict (winner or loser). Disagreement implies non-deterministic
    conflict resolution.
    """
    # digest -> validator -> verdict ("winner" | "loser")
    verdicts: dict = defaultdict(dict)
    locked_by_seen: dict = defaultdict(lambda: defaultdict(set))

    for ev in events:
        if isinstance(ev, WinnerLockAcquired):
            verdicts[ev.digest][ev.validator] = "winner"
        elif isinstance(ev, LoserDropped):
            verdicts[ev.digest][ev.validator] = "loser"
            locked_by_seen[ev.digest][ev.validator].add(ev.locked_by)

    anomalies: List[Anomaly] = []
    for digest, per_validator in verdicts.items():
        verdict_set = set(per_validator.values())
        # Treat all loser-* variants as equivalent for safety purposes: a tx
        # that was rejected on one validator and accepted on another is the bug.
        normalised = {("winner" if v == "winner" else "loser") for v in verdict_set}
        if len(normalised) > 1:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=f"Validators disagree on tx {digest}",
                    evidence={
                        "digest": digest,
                        "verdicts": per_validator,
                    },
                )
            )

        # Validators reporting locked_by for the same loser must agree on the
        # winner; disagreement is a real safety failure, hence FAIL.
        all_winners = set()
        for vset in locked_by_seen.get(digest, {}).values():
            all_winners.update(vset)
        if len(all_winners) > 1:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=(
                        f"Validators disagree on which tx won against loser "
                        f"{digest}"
                    ),
                    evidence={
                        "loser_digest": digest,
                        "locked_by_per_validator": {
                            v: sorted(s)
                            for v, s in locked_by_seen[digest].items()
                        },
                    },
                )
            )

    return CheckResult(
        name="B",
        description="Cross-validator agreement",
        items_checked=len(verdicts),
        anomalies=anomalies,
    )


# ---------- C: Executions match winners -----------------------------------

def check_losers_never_executed(events: Iterable) -> CheckResult:
    """A transaction recorded as a loser (any conflict variant) on a given
    validator must NEVER appear as Executed on that same validator. This is
    the headline safety invariant: a rejected tx that executes anyway is a
    direct double-spend.

    The complementary direction (every winner executes) is intentionally not
    checked — non-conflict-validated transactions (genesis, system) also
    emit `process_transaction succeeded`, so a missing winner is normal.
    """
    losers: dict = {}  # (validator, digest) -> True
    exec_fx: dict = {}  # (validator, digest) -> fx_digest

    for ev in events:
        if isinstance(ev, LoserDropped):
            losers[(ev.validator, ev.digest)] = True
        elif isinstance(ev, Executed):
            exec_fx[(ev.validator, ev.digest)] = ev.fx_digest

    anomalies: List[Anomaly] = []
    for vd in losers:
        if vd in exec_fx:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=(
                        f"Tx {vd[1]} rejected as loser on {vd[0]} but "
                        f"ALSO executed on {vd[0]} — double-spend leaked"
                    ),
                    evidence={
                        "validator": vd[0],
                        "digest": vd[1],
                        "fx_digest": exec_fx[vd],
                    },
                )
            )

    return CheckResult(
        name="C",
        description="Losers never executed",
        items_checked=len(losers),
        anomalies=anomalies,
    )


# ---------- D: Batch counts reconcile -------------------------------------

def check_batch_counts(events: Iterable) -> CheckResult:
    """For each validator, the total `num_dropped` across BatchSummary lines
    should be >= the count of individual LoserDropped events. `num_dropped`
    counts ALL post-consensus drops (owned-object lock conflicts plus
    validity-check, deny-check and input-extraction failures), whereas
    LoserDropped counts only the lock conflicts, so a small positive
    `dropped - losers` gap is expected. A negative gap (more loser lines than
    drops) or a large positive gap suggests the parser is missing loser lines.
    This is diagnostic only (WARN); it never fails the audit.

    We do NOT compare winners vs `num_retained`: the summary log only fires
    when `num_dropped > 0`, so its retained field only covers contentious
    commits and would underreport against the total Winner-event count.
    """
    per_v_loser = defaultdict(int)
    per_v_sum_dropped = defaultdict(int)

    for ev in events:
        if isinstance(ev, LoserDropped):
            per_v_loser[ev.validator] += 1
        elif isinstance(ev, BatchSummary):
            per_v_sum_dropped[ev.validator] += ev.num_dropped

    anomalies: List[Anomaly] = []
    validators = set(per_v_loser) | set(per_v_sum_dropped)
    for v in sorted(validators):
        l = per_v_loser.get(v, 0)
        d = per_v_sum_dropped.get(v, 0)
        if l != d:
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"{v}: loser events={l} vs sum(dropped)={d} "
                        f"(diff {l - d:+d})"
                    ),
                    evidence={
                        "validator": v,
                        "loser_events": l,
                        "dropped_total": d,
                    },
                )
            )

    return CheckResult(
        name="D",
        description="Dropped counts reconcile",
        items_checked=len(validators),
        anomalies=anomalies,
    )


# ---------- E: Stress / fullnode / validator consistency ------------------

def check_stress_consistency(
    validator_events: Iterable,
    fn_submissions: set,
    fn_final_failures: dict,
    fn_executed: dict,
) -> CheckResult:
    """Cross-check the fullnode's view (which transactions were submitted and
    what their terminal outcomes were) against the validator-side verdicts.

    Anomaly classes:
      - A digest that finalised on the fullnode (effects observed) but every
        validator recorded it as a loser → finality-reporting bug.
      - A digest that the fullnode declared as a final failure but at least
        one validator says it won → finality-reporting bug (opposite direction).
      - A digest submitted by the fullnode but never observed on any
        validator → completeness gap, not safety. Reported as WARN.
    """
    winners_any: set = set()
    losers_any: set = set()
    for ev in validator_events:
        if isinstance(ev, WinnerLockAcquired):
            winners_any.add(ev.digest)
        elif isinstance(ev, LoserDropped):
            losers_any.add(ev.digest)

    anomalies: List[Anomaly] = []

    for digest in fn_executed:
        if digest in losers_any and digest not in winners_any:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=(
                        f"Tx {digest} reported Executed by fullnode but "
                        f"recorded only as loser on validators"
                    ),
                    evidence={
                        "digest": digest,
                        "fn_effects_digest": fn_executed[digest],
                    },
                )
            )

    for digest, reason in fn_final_failures.items():
        if digest in winners_any:
            anomalies.append(
                Anomaly(
                    severity="FAIL",
                    message=(
                        f"Tx {digest} reported failed by fullnode but won on "
                        f"at least one validator"
                    ),
                    evidence={
                        "digest": digest,
                        "fn_reason": reason,
                    },
                )
            )

    missing_on_validators = fn_submissions - (winners_any | losers_any)
    if missing_on_validators:
        # Bucket by what the fullnode said happened to them.
        bucket_stale_object = []
        bucket_other_failure = []
        bucket_no_outcome = []
        for d in missing_on_validators:
            reason = fn_final_failures.get(d)
            if reason is None:
                bucket_no_outcome.append(d)
            elif "is not available for consumption, current version" in reason:
                bucket_stale_object.append(d)
            else:
                bucket_other_failure.append(d)

        # Pre-consensus stale-object rejections are benign — a separate safety
        # layer caught them before they reached consensus. Report informationally.
        if bucket_stale_object:
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"{len(bucket_stale_object)} submitted txs rejected "
                        f"pre-consensus (stale input object) — benign, "
                        f"pre-consensus safety layer caught them"
                    ),
                    evidence={
                        "count": len(bucket_stale_object),
                        "sample": sorted(bucket_stale_object)[:10],
                    },
                )
            )

        # Other terminal failures need closer inspection.
        if bucket_other_failure:
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"{len(bucket_other_failure)} submitted txs failed "
                        f"on fullnode for non-stale reasons and never reached "
                        f"validator post-consensus"
                    ),
                    evidence={
                        "count": len(bucket_other_failure),
                        "sample": sorted(bucket_other_failure)[:10],
                    },
                )
            )

        # No terminal outcome at all is the most suspicious — could be a
        # truncated log or a genuine completeness gap.
        if bucket_no_outcome:
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"{len(bucket_no_outcome)} submitted txs have no "
                        f"recorded terminal outcome on fullnode AND no "
                        f"validator post-consensus event — possible log "
                        f"truncation or pipeline gap"
                    ),
                    evidence={
                        "count": len(bucket_no_outcome),
                        "sample": sorted(bucket_no_outcome)[:10],
                    },
                )
            )

    return CheckResult(
        name="E",
        description="Fullnode / validator consistency",
        items_checked=len(fn_submissions),
        anomalies=anomalies,
    )


# ---------- F: Double-spend pair tracking ---------------------------------

def check_double_spend_pairs(
    double_spend_attempts: Iterable,
    validator_events: Iterable,
    pre_consensus_rejected: set = frozenset(),
) -> CheckResult:
    """Cross-reference the double-spend workload's pre-submission log against
    validator verdicts.

    The workload emits one `DoubleSpendAttempt(pair_id, gas_object,
    gas_version, digest, sink, ts)` per submitted tx. Attempts that share a
    `pair_id` contest the same gas coin and are *expected* to collide on the
    validator side. This check groups attempts by pair and, for each pair,
    reports how many submitted digests were observed by at least one validator
    and how the validator quorum split them between winner / loser / executed.

    A submitted digest that no validator observed is only a concern if it
    cannot be explained: the workload submits faster than the contested coin
    advances, so many attempts reference an already-consumed gas version and
    are rejected pre-consensus (stale object) before reaching the
    owned-object contest. `pre_consensus_rejected` carries the digests known
    (from the stress error and/or fullnode terminal failure) to have been
    rejected at submission; those are accounted for as benign and only the
    unexplained remainder is flagged.

    Anomaly classes:
      - WARN: a pair has missing digests NOT explained by a pre-consensus
              rejection (a genuine completeness gap).
      - WARN: a pair has zero winners despite having multiple attempts
              (the contest never produced a winning tx on any validator).
      - INFO: total missing digests accounted for as pre-consensus rejections.
    """
    # pair_id -> list of attempts (preserve order for diagnostics)
    by_pair: dict = defaultdict(list)
    # Track first-seen attempt per digest to keep evidence compact.
    digest_to_attempt: dict = {}
    for ev in double_spend_attempts:
        by_pair[ev.pair_id].append(ev)
        digest_to_attempt.setdefault(ev.digest, ev)

    # Validator-side digest sets (built once).
    winners_any: set = set()
    losers_any: set = set()
    executed_any: set = set()
    for ev in validator_events:
        if isinstance(ev, WinnerLockAcquired):
            winners_any.add(ev.digest)
        elif isinstance(ev, LoserDropped):
            losers_any.add(ev.digest)
        elif isinstance(ev, Executed):
            executed_any.add(ev.digest)

    seen_on_validator = winners_any | losers_any

    anomalies: List[Anomaly] = []
    total_missing = 0
    total_accounted = 0
    for pair_id in sorted(by_pair):
        attempts = by_pair[pair_id]
        unique_digests = {a.digest for a in attempts}
        gas_objects = {a.gas_object for a in attempts}
        winners = unique_digests & winners_any
        losers = unique_digests & losers_any
        executed = unique_digests & executed_any
        missing = unique_digests - seen_on_validator
        accounted = missing & pre_consensus_rejected
        unexplained = missing - pre_consensus_rejected
        total_missing += len(missing)
        total_accounted += len(accounted)

        if unexplained:
            # Show a small sample with their (gas_object, gas_version) so the
            # user can find the matching lines in the stress log.
            sample = sorted(unexplained)[:6]
            sample_detail = [
                {
                    "tx_digest": d,
                    "gas_object": digest_to_attempt[d].gas_object,
                    "gas_version": digest_to_attempt[d].gas_version,
                }
                for d in sample
            ]
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"pair {pair_id}: {len(unexplained)}/"
                        f"{len(unique_digests)} submitted digests never "
                        f"observed on any validator and not explained by a "
                        f"pre-consensus rejection"
                    ),
                    evidence={
                        "pair_id": pair_id,
                        "gas_objects": sorted(gas_objects),
                        "submitted_unique": len(unique_digests),
                        "missing_count": len(missing),
                        "accounted_pre_consensus": len(accounted),
                        "unexplained_count": len(unexplained),
                        "unexplained_sample": sample_detail,
                    },
                )
            )

        if len(unique_digests) >= 2 and not winners:
            anomalies.append(
                Anomaly(
                    severity="WARN",
                    message=(
                        f"pair {pair_id}: {len(unique_digests)} distinct "
                        f"digests submitted but no winner recorded on any "
                        f"validator"
                    ),
                    evidence={
                        "pair_id": pair_id,
                        "gas_objects": sorted(gas_objects),
                        "submitted_unique": len(unique_digests),
                        "losers_count": len(losers),
                        "executed_count": len(executed),
                    },
                )
            )

    if total_accounted:
        anomalies.append(
            Anomaly(
                severity="INFO",
                message=(
                    f"{total_accounted}/{total_missing} digests missing from "
                    f"validator post-consensus logs accounted for as "
                    f"pre-consensus rejections (stale gas object) — benign"
                ),
                evidence={
                    "accounted_pre_consensus": total_accounted,
                    "total_missing": total_missing,
                },
            )
        )

    return CheckResult(
        name="F",
        description="Double-spend pair tracking",
        items_checked=len(by_pair),
        anomalies=anomalies,
    )
