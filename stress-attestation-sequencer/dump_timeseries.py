#!/usr/bin/env python3
"""dump_timeseries.py — scrape one run window from Prometheus into a raw JSON.

Stores the underlying series verbatim — cumulative histogram buckets
(+ _count/_sum) and raw counters/gauges — with NO rate()/histogram_quantile()/
aggregation baked in. Everything (any rate window, any quantile, per-validator
breakdowns, correct cross-run aggregation by pooling raw histograms) is
reconstructed from this offline by aggregate.py / plot.py. Each entry is the raw
query_range result: one series per full label set (le, host, name, …),
values = [[ts, "v"], …].

Pure stdlib (urllib/json) — runs on system python3, no venv.

Usage:  dump_timeseries.py <label> <start_epoch> <end_epoch> <step_s> <out.json>
Env:    PROM (Prometheus base URL), CFG_* (recorded under "config" in the JSON
        so the aggregator can flag pooling across mismatched configs).

Standalone use (e.g. re-scrape a past window without re-running the experiment):
  PROM=http://localhost:9090 CFG_workload=owned \
    python3 dump_timeseries.py mylabel 1781770000 1781770030 1 /tmp/out.json
"""

import json
import os
import sys
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor

label, start, end, step, out = sys.argv[1:6]
prom = os.environ["PROM"]
# Per-run config (so the aggregator can flag pooling across mismatched configs).
config = {k[4:]: v for k, v in os.environ.items() if k.startswith("CFG_")}

# name -> raw PromQL selector. Histograms keep their le+host labels; counters
# stay cumulative (compute rate() offline); CPU is scoped to the node containers
# (validators + fullnodes) to bound cardinality.
metrics = {}
for base in (
    "validator_attestation_latency",  # pre-consensus dry-run (V2): total
    "validator_attestation_queue_wait",  # V2 split: wait on spawn_blocking pool
    "validator_attestation_execution_latency",  # V2 split: dry-run Move-VM exec
    "validator_attestation_async_resume_latency",  # V2 split: async reschedule after join
    "validator_transaction_execution_latency",  # validator-internal pipeline
    "authority_state_internal_execution_latency",  # pure VM execution
    # Same, user transactions only. The unqualified metric above also counts
    # the per-commit system transactions, which are far cheaper and outnumber
    # user ones once a run's throughput drops, so its mean says little about
    # what a user transaction actually cost to execute.
    "authority_state_internal_execution_latency_user",
    "transaction_driver_settlement_finality_latency",  # client-side (fullnode)
    "transaction_driver_submit_transaction_latency",  # client-side (fullnode)
    "post_consensus_validation_latency",  # post-consensus validation pass
    "checkpoint_creation_latency",  # commit-created -> checkpoint-built lag (SECONDS)
    "execution_queueing_delay_s",  # execution-driver queueing delay
    "attested_computation_units",  # V2 attestation estimate
    "actual_computation_units",  # measured at execution
    "actual_to_attested_computation_units_ratio",  # attestation accuracy
    "consensus_handler_scheduled_transactions_per_object_per_commit",  # sched/obj/commit
):
    metrics[f"{base}_bucket"] = f"{base}_bucket"
    metrics[f"{base}_count"] = f"{base}_count"
    metrics[f"{base}_sum"] = f"{base}_sum"
# deferral-rounds histogram (per-tx rounds spent deferred; cancellation fires
# when this exceeds max_deferral_rounds — so its p99 tells you how close you are).
for sfx in ("bucket", "count", "sum"):
    metrics[f"consensus_handler_transaction_deferral_rounds_{sfx}"] = (
        f"consensus_handler_transaction_deferral_rounds_{sfx}"
    )
# raw counters / gauges
metrics["transactions_included_in_checkpoint"] = "transactions_included_in_checkpoint"
metrics["validator_attestations_total"] = "validator_attestations_total"
# congestion-control counters (deferred ⊇ congested; cancelled = deferred past
# the round limit). Cumulative — compute rate() offline.
metrics["consensus_handler_deferred_transactions"] = (
    "consensus_handler_deferred_transactions"
)
metrics["consensus_handler_congested_transactions"] = (
    "consensus_handler_congested_transactions"
)
metrics["consensus_handler_cancelled_transactions"] = (
    "consensus_handler_cancelled_transactions"
)
# txns dropped by post-consensus validation (dedup / already-executed / validity
# / attestation / lock-conflict). A deferred tx that self-conflicts on its own
# prior-round lock surfaces here instead of being re-scheduled, so this rate
# tracking the deferred rate signals deferred txns are being dropped, not rolled.
metrics["consensus_handler_validation_dropped_transactions"] = (
    "consensus_handler_validation_dropped_transactions"
)
# max scheduled per-object cost in a commit (compare vs the per-commit budget).
metrics["consensus_handler_max_congestion_control_object_costs"] = (
    "consensus_handler_max_congestion_control_object_costs"
)
# post-consensus load shedding (PR #11301): heavy execution -> overload -> user
# txns deterministically dropped AFTER consensus by the stake-weighted quorum shed
# percentage. The drop counter is cumulative (rate() offline); the percentages and
# the overload flag are gauges (see GAUGES below).
metrics["consensus_handler_load_shedding_dropped_transactions"] = (
    "consensus_handler_load_shedding_dropped_transactions"
)
metrics["consensus_handler_load_shedding_percentage"] = (
    "consensus_handler_load_shedding_percentage"  # enforced quorum (2f+1) shed %
)
metrics["authority_load_shedding_percentage"] = (
    "authority_load_shedding_percentage"  # this validator's locally computed %
)
metrics["consensus_queue_load_shedding_percentage"] = (
    "consensus_queue_load_shedding_percentage"  # separate consensus-queue signal
)
# --- pre-consensus admission-control shedding -------------------------------
# Transactions rejected BEFORE consensus by check_system_overload (validator_v2),
# so the post-consensus percentages above can read 0 while these fire — e.g. when
# the submit_semaphore has no permits (surfaces under transaction_overload_sources
# with source="consensus"). Cumulative counters (rate() offline). The labeled
# *_during_overload / *_sources counters carry an error_type/source label; store
# the per-host TOTAL (summed across the label) so plot.py's per-validator network
# collapse is a clean rate. transaction_overload_sources is ALSO kept raw
# (per-source) for drill-down — e.g. isolating the semaphore ("consensus") source.
metrics["validator_service_num_rejected_tx_during_overload"] = (
    "sum by (host) (validator_service_num_rejected_tx_during_overload)"
)
metrics["transaction_overload_sources"] = "sum by (host) (transaction_overload_sources)"
metrics["transaction_overload_sources_by_source"] = "transaction_overload_sources"
# committed subdags = consensus commits, counted per leader. Summed per host, its
# rate is the commit rate — what a per-object per-commit limit is spent against, and
# what turns a limit in units per commit into units per second.
metrics["consensus_committed_subdags"] = "sum by (host) (consensus_committed_subdags)"
# last leader round the consensus handler processed. A skipped leader round
# advances it without producing a commit, so subtracting the committed-subdag
# increase gives the number of skipped rounds. Deferral limits are a
# commit-round difference, so a skipped round spends a transaction's deferral
# budget without giving it a scheduling attempt.
metrics["consensus_handler_leader_round"] = "consensus_handler_leader_round"
# consensus in-flight transactions (num_inflight): the value graduated / max_pending
# shedding compares against max_pending_transactions. sequencing_certificate_inflight
# is an IntGaugeVec by tx_type; sum by host = num_inflight per validator. It's a GAUGE
# (rises and falls with the queue), so it's excluded from reset-trimming (see GAUGES).
metrics["sequencing_certificate_inflight"] = (
    "sum by (host) (sequencing_certificate_inflight)"
)
metrics["container_cpu_usage_seconds_total"] = (
    'container_cpu_usage_seconds_total{name=~"validator-.*|fullnode-.*"}'
)
# resource usage: per-container memory (cadvisor, scoped) + host CPU (node-exporter).
metrics["container_memory_rss"] = (
    'container_memory_rss{name=~"validator-.*|fullnode-.*"}'
)
metrics["node_cpu_seconds_total"] = "node_cpu_seconds_total"
# execution pipeline throughput / backpressure (does attestation starve execution?).
metrics["execution_driver_executed_transactions"] = (
    "execution_driver_executed_transactions"
)
metrics["execution_driver_dispatch_queue"] = "execution_driver_dispatch_queue"
metrics["execution_cache_backpressure_status"] = "execution_cache_backpressure_status"
metrics["execution_cache_backpressure_toggles"] = "execution_cache_backpressure_toggles"
metrics["transaction_manager_num_pending_certificates"] = (
    "transaction_manager_num_pending_certificates"
)
# attestation health (V2): task panics + soft-lock-conflict rejections.
metrics["validator_attestation_task_panics"] = "validator_attestation_task_panics"
metrics["validator_service_num_rejected_tx_soft_lock_conflict"] = (
    "validator_service_num_rejected_tx_soft_lock_conflict"
)
# safety / fork detection — must stay 0 for the run to be valid.
metrics["global_state_hash_inconsistent_state"] = "global_state_hash_inconsistent_state"
metrics["remote_checkpoint_forks"] = "remote_checkpoint_forks"
metrics["split_brain_checkpoint_forks"] = "split_brain_checkpoint_forks"
metrics["total_client_double_spend_attempts_detected"] = (
    "total_client_double_spend_attempts_detected"
)

# Cumulative counters/histograms reset to 0 when a process restarts. Because the
# Prometheus TSDB is kept across runs (so A+B coexist in Grafana), Run B reuses
# Run A's series labels: Prometheus carries Run A's last (higher) value into the
# START of Run B's window before the fresh process's series take over — i.e. a
# reset WITHIN the window that makes naive last-first go negative. Drop every
# sample up to and including the LAST such reset, so last-first over the kept
# samples = this process's in-window increase (matches PromQL increase()).
# GAUGES legitimately rise and fall (not monotonic), so they are left raw.
GAUGES = {
    "consensus_handler_max_congestion_control_object_costs",
    "execution_cache_backpressure_status",
    "execution_driver_dispatch_queue",
    "transaction_manager_num_pending_certificates",
    "container_memory_rss",
    "global_state_hash_inconsistent_state",
    # load-shedding percentages rise and fall (not monotonic).
    "consensus_handler_load_shedding_percentage",
    "authority_load_shedding_percentage",
    "consensus_queue_load_shedding_percentage",
    # num_inflight consensus queue depth rises and falls (not monotonic).
    "sequencing_certificate_inflight",
}


def trim_after_last_reset(values):
    last = 0
    for i in range(1, len(values)):
        if float(values[i][1]) < float(values[i - 1][1]):
            last = i
    return values[last:]


def fetch(item):
    name, q = item
    url = (
        prom
        + "/api/v1/query_range?"
        + urllib.parse.urlencode({"query": q, "start": start, "end": end, "step": step})
    )
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            result = json.load(r).get("data", {}).get("result", [])
        if name not in GAUGES:
            for s_ in result:
                if s_.get("values"):
                    s_["values"] = trim_after_last_reset(s_["values"])
        return name, result
    except Exception as e:  # noqa: BLE001
        return name, {"error": str(e)}


# The queries are independent and I/O-bound, so fetch them concurrently — run
# sequentially the ~70 of them dominate the dump's wall time. 8 workers stays
# well below Prometheus's default query concurrency limit (20). pool.map keeps
# the metrics-dict order, so the output JSON is stable.
with ThreadPoolExecutor(max_workers=8) as pool:
    series = dict(pool.map(fetch, metrics.items()))

with open(out, "w") as f:
    json.dump(
        {
            "label": label,
            "start_epoch": int(start),
            "end_epoch": int(end),
            "step_seconds": int(step),
            "config": config,
            "series": series,
        },
        f,
        indent=2,
    )
