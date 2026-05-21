// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::{max, min},
    hash::Hasher,
    sync::{
        Weak,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iota_config::node::AuthorityOverloadConfig;
use iota_metrics::monitored_scope;
use iota_types::{
    digests::TransactionDigest,
    error::{IotaError, IotaResult},
    fp_bail,
};
use tokio::time::sleep;
use tracing::{debug, info};
use twox_hash::XxHash64;

use crate::{authority::AuthorityState, consensus_adapter::ConsensusAdapter};

#[derive(Default)]
pub struct AuthorityOverloadInfo {
    /// Whether the authority is overloaded.
    pub is_overload: AtomicBool,

    /// The locally computed percentage of transactions this authority would
    /// drop. This is the *output* of this authority's overload monitor; it is
    /// distinct from the quorum-determined percentage actually enforced in the
    /// post-consensus load-shedding path. It is also read back on the next
    /// iteration of `check_execution_overload` as the feedback term for the
    /// latency-based controller.
    pub local_load_shedding_percentage: AtomicU32,
}

impl AuthorityOverloadInfo {
    pub fn set_overload(&self, local_load_shedding_percentage: u32) {
        self.is_overload.store(true, Ordering::Relaxed);
        self.local_load_shedding_percentage
            .store(min(local_load_shedding_percentage, 100), Ordering::Relaxed);
    }

    pub fn clear_overload(&self) {
        self.is_overload.store(false, Ordering::Relaxed);
        self.local_load_shedding_percentage
            .store(0, Ordering::Relaxed);
    }
}

const STEADY_OVERLOAD_REDUCTION_PERCENTAGE: u32 = 10;
const EXECUTION_RATE_RATIO_FOR_COMPARISON: f64 = 0.95;
const ADDITIONAL_LOAD_SHEDDING: f64 = 0.02;

// The update interval of the random seed used to determine whether a txn should
// be rejected.
const SEED_UPDATE_DURATION_SECS: u64 = 30;

// Monitors the overload signals in `authority_state` periodically, and updates
// its `overload_info` when the signals indicates overload.
pub async fn overload_monitor(
    authority_state: Weak<AuthorityState>,
    config: AuthorityOverloadConfig,
) {
    info!("Starting system overload monitor.");

    loop {
        let authority_exist = check_execution_overload(&authority_state, &config);
        if !authority_exist {
            // `authority_state` doesn't exist anymore. Quit overload monitor.
            break;
        }
        sleep(config.overload_monitor_interval).await;
    }

    info!("Shut down system overload monitor.");
}

/// Periodically refreshes the `consensus_queue_load_shedding_percentage`
/// metric so it tracks the current consensus queue depth even when no
/// gRPC traffic is arriving (which would otherwise be the only path that
/// updates the metric, via `check_consensus_queue_graduated_limits` on
/// `AuthorityState`). Used only in the certificate-less (pcool / white-flag)
/// mode.
pub async fn consensus_queue_overload_monitor(
    authority_state: Weak<AuthorityState>,
    consensus_adapter: Weak<ConsensusAdapter>,
    interval: Duration,
) {
    info!("Starting consensus queue overload monitor.");

    loop {
        let (Some(state), Some(adapter)) = (authority_state.upgrade(), consensus_adapter.upgrade())
        else {
            // Either `authority_state` or `consensus_adapter` doesn't exist
            // anymore. Quit monitor.
            break;
        };

        let num_inflight_txs = adapter.num_inflight_transactions() as usize;
        let shedding_pct = compute_graduated_load_shedding_percentage(
            num_inflight_txs,
            adapter.max_pending_transactions(),
            adapter.graduated_load_shedding_soft_limit_pct(),
        );
        state
            .metrics
            .consensus_queue_load_shedding_percentage
            .set(shedding_pct as i64);

        sleep(interval).await;
    }

    info!("Shut down consensus queue overload monitor.");
}

// Checks authority overload signals, and updates authority's `overload_info`.
// Returns whether the authority state exists.
fn check_execution_overload(
    authority_state: &Weak<AuthorityState>,
    config: &AuthorityOverloadConfig,
) -> bool {
    let _scope = monitored_scope("OverloadMonitor::check_authority_overload");
    let authority_arc = authority_state.upgrade();
    if authority_arc.is_none() {
        // `authority_state` doesn't exist anymore.
        return false;
    }

    let authority = authority_arc.unwrap();
    let queueing_latency = authority
        .metrics
        .execution_queueing_latency
        .latency()
        .unwrap_or_default();
    let txn_ready_rate = authority.metrics.txn_ready_rate_tracker.lock().rate();
    let execution_rate = authority.metrics.execution_rate_tracker.lock().rate();
    let inflight_queue_len = authority.transaction_manager().inflight_queue_len();
    let cache_pending_count = authority
        .get_cache_commit()
        .approximate_pending_transaction_count() as usize;

    // Surface the rate and cache-pressure raw inputs to the load-shedding
    // calculation as gauges so dashboards can answer "why is this source's
    // percentage what it is?". Queueing latency and inflight queue length are
    // already exposed via `execution_queueing_delay_s` and
    // `transaction_manager_num_pending_certificates` respectively.
    authority
        .metrics
        .overload_signal_txn_ready_rate_tps
        .set(txn_ready_rate as i64);
    authority
        .metrics
        .overload_signal_execution_rate_tps
        .set(execution_rate as i64);
    authority
        .metrics
        .overload_signal_cache_pending_count
        .set(cache_pending_count as i64);

    debug!(
        "Check authority overload signal, queueing latency {:?}, ready rate {:?}, execution rate {:?}, inflight queue len {:?}, cache pending count {:?}.",
        queueing_latency, txn_ready_rate, execution_rate, inflight_queue_len, cache_pending_count
    );

    // Feedback term: the locally computed percentage this monitor produced on
    // its previous iteration. We use the *local* value (not the quorum value
    // enforced post-consensus) because `txn_ready_rate` reflects the load this
    // authority itself admitted, so the controller must close the loop on its
    // own last decision to compound shedding over multiple iterations and to
    // ratchet down gradually as latency recovers.
    let current_local_load_shedding_percentage = authority
        .overload_info
        .local_load_shedding_percentage
        .load(Ordering::Relaxed);

    let (_, latency_based_percentage) = compute_latency_load_shedding_percentage(
        config,
        current_local_load_shedding_percentage,
        queueing_latency,
        txn_ready_rate,
        execution_rate,
    );

    let queue_based_percentage = compute_graduated_load_shedding_percentage(
        inflight_queue_len,
        config.max_transaction_manager_queue_length,
        config.max_transaction_manager_queue_length_soft_limit_pct(),
    );

    let cache_config = &authority.config.execution_cache_config.writeback_cache;
    let cache_based_percentage = compute_graduated_load_shedding_percentage(
        cache_pending_count,
        cache_config.backpressure_threshold() as usize,
        cache_config.backpressure_soft_limit_pct(),
    );
    authority
        .metrics
        .cache_backpressure_load_shedding_percentage
        .set(cache_based_percentage as i64);
    authority
        .metrics
        .authority_load_shedding_source
        .with_label_values(&["latency"])
        .set(latency_based_percentage as i64);
    authority
        .metrics
        .authority_load_shedding_source
        .with_label_values(&["queue_length"])
        .set(queue_based_percentage as i64);
    authority
        .metrics
        .authority_load_shedding_source
        .with_label_values(&["cache_backpressure"])
        .set(cache_based_percentage as i64);

    // The final load shedding percentage combines three signals:
    //   - latency/rate-based, from execution queueing latency,
    //   - queue-length-based, from the txn manager's inflight queue,
    //   - cache-backpressure-based, from the writeback cache's pending count.
    //
    // All three are correlated in steady state — by Little's Law,
    // `inflight_queue_len ≈ txn_ready_rate × queueing_latency`; cache pending
    // count tracks uncommitted writes which accumulate when execution outpaces
    // checkpoint flush — so they are combined with `max` rather than summed,
    // to avoid double-counting. Under transients they diverge: queue length
    // reacts to arrival bursts before averaged latency does, latency catches
    // sustained slow execution even when queue depth is modest, and cache
    // pressure shows up when checkpoint flush stalls even if execution itself
    // is keeping up. Each therefore guards a different failure mode.
    let load_shedding_percentage = max(
        max(latency_based_percentage, queue_based_percentage),
        cache_based_percentage,
    );
    let is_overload = load_shedding_percentage > 0;

    if is_overload {
        authority
            .overload_info
            .set_overload(load_shedding_percentage);
    } else {
        authority.overload_info.clear_overload();
    }

    authority
        .metrics
        .authority_overload_status
        .set(is_overload as i64);
    authority
        .metrics
        .authority_load_shedding_percentage
        .set(load_shedding_percentage as i64);
    true
}

// Calculates the percentage of transactions to drop in order to reduce
// execution queue. Returns the integer percentage between 0 and 100.
fn calculate_load_shedding_percentage(txn_ready_rate: f64, execution_rate: f64) -> u32 {
    // When transaction ready rate is practically 0, we aren't adding more load to
    // the execution driver, so no shedding.
    // TODO: consensus handler or transaction manager can also be overloaded.
    if txn_ready_rate < 1e-10 {
        return 0;
    }

    // Deflate the execution rate to account for the case that execution_rate is
    // close to txn_ready_rate.
    if execution_rate * EXECUTION_RATE_RATIO_FOR_COMPARISON > txn_ready_rate {
        return 0;
    }

    // In order to maintain execution queue length, we need to drop at least (1 -
    // executionRate / readyRate). To reduce the queue length, here we add 10%
    // more transactions to drop.
    (((1.0 - execution_rate * EXECUTION_RATE_RATIO_FOR_COMPARISON / txn_ready_rate)
        + ADDITIONAL_LOAD_SHEDDING)
        .min(1.0)
        * 100.0)
        .round() as u32
}

// Given overload signals (`queueing_latency`, `txn_ready_rate`,
// `execution_rate`), return whether the authority server should enter load
// shedding mode, and how much percentage of transactions to drop. Note that the
// final load shedding percentage should also take the current load shedding
// percentage into consideration. If we are already shedding 40% load, based on
// the current txn_ready_rate and execution_rate, we need to shed 10% more, the
// outcome is that we need to shed 40% + (1 - 40%) * 10% = 46%.
// When txn_ready_rate is less than execution_rate, we gradually reduce load
// shedding percentage until the queueing latency is back to normal.
fn compute_latency_load_shedding_percentage(
    config: &AuthorityOverloadConfig,
    current_load_shedding_percentage: u32,
    queueing_latency: Duration,
    txn_ready_rate: f64,
    execution_rate: f64,
) -> (bool, u32) {
    // First, we calculate based on the current `txn_ready_rate` and
    // `execution_rate`, what's the percentage of traffic to shed from
    // `txn_ready_rate`.
    let additional_load_shedding_percentage;
    if queueing_latency > config.execution_queue_latency_hard_limit {
        let calculated_load_shedding_percentage =
            calculate_load_shedding_percentage(txn_ready_rate, execution_rate);

        additional_load_shedding_percentage = if calculated_load_shedding_percentage > 0
            || txn_ready_rate >= config.safe_transaction_ready_rate as f64
        {
            max(
                calculated_load_shedding_percentage,
                config.min_load_shedding_percentage_above_hard_limit,
            )
        } else {
            0
        };
    } else if queueing_latency > config.execution_queue_latency_soft_limit {
        additional_load_shedding_percentage =
            calculate_load_shedding_percentage(txn_ready_rate, execution_rate);
    } else {
        additional_load_shedding_percentage = 0;
    }

    // Next, we calculate the new load shedding percentage.
    let load_shedding_percentage = if additional_load_shedding_percentage > 0 {
        // When we need to shed more load, since the `txn_ready_rate` is already
        // influenced by `current_load_shedding_percentage`, we need to
        // calculate the new load shedding percentage from
        // `current_load_shedding_percentage` and
        // `additional_load_shedding_percentage`.
        current_load_shedding_percentage
            + (100 - current_load_shedding_percentage) * additional_load_shedding_percentage / 100
    } else if txn_ready_rate > config.safe_transaction_ready_rate as f64
        && current_load_shedding_percentage > 10
    {
        // We don't need to shed more load. However, the enqueue rate is still not
        // minimal. We gradually reduce load shedding percentage (10% at a time)
        // to gracefully accept more load.
        current_load_shedding_percentage - STEADY_OVERLOAD_REDUCTION_PERCENTAGE
    } else {
        // The current transaction ready rate is considered very low. Turn off load
        // shedding mode.
        0
    };

    let load_shedding_percentage = min(
        load_shedding_percentage,
        config.max_load_shedding_percentage,
    );
    let overload_status = load_shedding_percentage > 0;
    (overload_status, load_shedding_percentage)
}

/// Return true if we should reject the txn with `tx_digest`.
pub(crate) fn should_reject_tx(
    load_shedding_percentage: u32,
    tx_digest: TransactionDigest,
    temporal_seed: u64,
) -> bool {
    // TODO: we also need to add a secret salt (e.g. first consensus commit in the
    // current epoch), to prevent gaming the system.
    let mut hasher = XxHash64::with_seed(temporal_seed);
    hasher.write(tx_digest.inner());
    let value = hasher.finish();
    value % 100 < load_shedding_percentage as u64
}

/// Checks if we can accept the transaction with `tx_digest`.
pub fn overload_monitor_accept_tx(
    load_shedding_percentage: u32,
    tx_digest: TransactionDigest,
) -> IotaResult {
    // Derive a random seed from the epoch time for transaction selection. Changing
    // the seed every `SEED_UPDATE_DURATION_SECS` interval allows rejected
    // transaction's retry to have a chance to go through in the future.
    // Also, using the epoch time instead of randomly generating a seed allows that
    // all validators makes the same decision.
    let temporal_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("IOTA did not exist prior to 1970")
        .as_secs()
        / SEED_UPDATE_DURATION_SECS;

    if should_reject_tx(load_shedding_percentage, tx_digest, temporal_seed) {
        // TODO: using `SEED_UPDATE_DURATION_SECS` is a safe suggestion that the time
        // based seed is definitely different by then. However, a shorter
        // suggestion may be available.
        fp_bail!(IotaError::ValidatorOverloadedRetryAfter {
            retry_after_secs: SEED_UPDATE_DURATION_SECS
        });
    }
    Ok(())
}

/// Computes the graduated load shedding percentage based on the current value
/// relative to its hard limit. Returns 0 if `current` is at or below the soft
/// limit (computed as `hard_limit * soft_limit_pct / 100`), linearly scales
/// from 0% to 100% between soft and hard limits, and returns 100% if `current`
/// is at or above `hard_limit`.
///
/// `soft_limit_pct` is expected to be in `[0, 100]`. Values above 100 are
/// clamped to 100 in release builds and trigger a debug assertion in debug
/// builds.
///
/// Setting `soft_limit_pct = 100` degenerates into a hard binary cutoff: no
/// shedding below `hard_limit`, full (100%) shedding at and above it.
///
/// NOTE: `soft_limit` is computed via integer division `hard_limit *
/// soft_limit_pct / 100`, so it floors. This is negligible for typical
/// queue sizes (thousands).
pub(crate) fn compute_graduated_load_shedding_percentage(
    current: usize,
    hard_limit: usize,
    soft_limit_pct: u32,
) -> u32 {
    debug_assert!(
        soft_limit_pct <= 100,
        "soft_limit_pct must be <= 100, got {soft_limit_pct}"
    );
    // Clamp `soft_limit_pct` to 100% to be safe in release builds.
    let soft_limit_pct = soft_limit_pct.min(100);
    // Convert soft limit percentage to absolute soft limit.
    let soft_limit = hard_limit * soft_limit_pct as usize / 100;

    // At or above hard limit, shed at maximum percentage.
    // WARN: this hard limit check must come BEFORE the soft limit check.
    // When `soft_limit_pct == 100`, soft_limit == hard_limit, and at `current ==
    // hard_limit`, we want 100% shedding (binary cutoff behavior), not 0%.
    // Swapping the order would incorrectly return 0 in this degenerate case.
    if current >= hard_limit {
        return 100;
    }

    // No shedding below or at soft limit.
    if current <= soft_limit {
        return 0;
    }

    // The two early returns above imply that at this point,
    // `soft_limit < current < hard_limit`, so the following two
    // subtraction results are guaranteed to be strictly > 0.
    let range = hard_limit - soft_limit;
    let excess = current - soft_limit;

    // Linear interpolation: 0% at `soft_limit`, 100% at `hard_limit`.
    (excess * 100 / range) as u32
}

#[cfg(test)]
#[expect(clippy::disallowed_methods)] // allow unbounded_channel() since tests are simulating txn manager execution
// driver interaction.
mod tests {
    use std::sync::Arc;

    use iota_macros::sim_test;
    use rand::{
        Rng, SeedableRng,
        rngs::{OsRng, StdRng},
    };
    use tokio::{
        sync::{
            mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
            oneshot,
        },
        task::JoinHandle,
        time::{Instant, MissedTickBehavior, interval},
    };

    use super::*;
    use crate::authority::test_authority_builder::TestAuthorityBuilder;

    #[test]
    fn test_authority_overload_info() {
        let overload_info = AuthorityOverloadInfo::default();
        assert!(!overload_info.is_overload.load(Ordering::Relaxed));
        assert_eq!(
            overload_info
                .local_load_shedding_percentage
                .load(Ordering::Relaxed),
            0
        );

        {
            overload_info.set_overload(20);
            assert!(overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .local_load_shedding_percentage
                    .load(Ordering::Relaxed),
                20
            );
        }

        // Tests that load shedding percentage can't go beyond 100%.
        {
            overload_info.set_overload(110);
            assert!(overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .local_load_shedding_percentage
                    .load(Ordering::Relaxed),
                100
            );
        }

        {
            overload_info.clear_overload();
            assert!(!overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .local_load_shedding_percentage
                    .load(Ordering::Relaxed),
                0
            );
        }
    }

    #[test]
    fn test_calculate_load_shedding_ratio() {
        assert_eq!(calculate_load_shedding_percentage(95.0, 100.1), 0);
        assert_eq!(calculate_load_shedding_percentage(95.0, 100.0), 2);
        assert_eq!(calculate_load_shedding_percentage(100.0, 100.0), 7);
        assert_eq!(calculate_load_shedding_percentage(110.0, 100.0), 16);
        assert_eq!(calculate_load_shedding_percentage(180.0, 100.0), 49);
        assert_eq!(calculate_load_shedding_percentage(100.0, 0.0), 100);
        assert_eq!(calculate_load_shedding_percentage(0.0, 1.0), 0);
    }

    #[test]
    fn test_check_overload_signals() {
        let config = AuthorityOverloadConfig {
            execution_queue_latency_hard_limit: Duration::from_secs(10),
            execution_queue_latency_soft_limit: Duration::from_secs(1),
            max_load_shedding_percentage: 90,
            ..Default::default()
        };

        // When execution queueing latency is within soft limit, don't start overload
        // protection.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_millis(500),
                1000.0,
                10.0
            ),
            (false, 0)
        );

        // When execution queueing latency hits soft limit and execution rate is higher,
        // don't start overload protection.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(2),
                100.0,
                120.0
            ),
            (false, 0)
        );

        // When execution queueing latency hits soft limit, but not hard limit, start
        // overload protection.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(2),
                100.0,
                100.0
            ),
            (true, 7)
        );

        // When execution queueing latency hits hard limit, start more aggressive
        // overload protection.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(11),
                100.0,
                100.0
            ),
            (true, 50)
        );

        // When execution queueing latency hits hard limit and calculated shedding
        // percentage is higher than
        // min_load_shedding_percentage_above_hard_limit.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(11),
                240.0,
                100.0
            ),
            (true, 62)
        );

        // When execution queueing latency hits hard limit, but transaction ready rate
        // is within safe_transaction_ready_rate, don't start overload protection.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(11),
                20.0,
                100.0
            ),
            (false, 0)
        );

        // Maximum transactions shed is cap by `max_load_shedding_percentage` config.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                0,
                Duration::from_secs(11),
                100.0,
                0.0
            ),
            (true, 90)
        );

        // When the system is already shedding 50% of load, and the current txn ready
        // rate and execution rate require another 20%, the final shedding rate
        // is 60%.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                50,
                Duration::from_secs(2),
                116.0,
                100.0
            ),
            (true, 60)
        );

        // Load shedding percentage is gradually reduced when txn ready rate is lower
        // than execution rate.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                90,
                Duration::from_secs(2),
                200.0,
                300.0
            ),
            (true, 80)
        );

        // When queueing delay is above hard limit, we shed additional 50% every time.
        assert_eq!(
            compute_latency_load_shedding_percentage(
                &config,
                50,
                Duration::from_secs(11),
                100.0,
                100.0
            ),
            (true, 75)
        );
    }

    /// Tests [`compute_graduated_load_shedding_percentage`]:
    /// - 0% at or below the soft limit (`hard_limit * soft_limit_pct / 100`)
    /// - linear scaling between soft and hard limits
    /// - 100% at or above the hard limit
    /// - degenerate cases for `soft_limit_pct = 0` and `soft_limit_pct = 100`
    #[test]
    fn test_compute_graduated_load_shedding_percentage() {
        let hard_limit = 20_000;
        let soft_limit_pct = 50;
        let soft_limit = hard_limit * soft_limit_pct as usize / 100; // 10_000

        // Below and at soft limit: no shedding.
        for current in [0, soft_limit - 1, soft_limit] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(current, hard_limit, soft_limit_pct),
                0,
                "no shedding expected at or below soft limit ({current} <= {soft_limit})",
            );
        }

        // Linear scaling between soft and hard limits:
        //  - At 25% of range (12_500): 100 * 2_500 / 10_000 = 25
        //  - At midpoint (15_000):     100 * 5_000 / 10_000 = 50
        //  - At 75% of range (17_500): 100 * 7_500 / 10_000 = 75
        //  - Just below hard limit:    100 * 9_999 / 10_000 = 99
        for (current, expected_pct) in [
            (12_500, 25),
            (15_000, 50),
            (17_500, 75),
            (hard_limit - 1, 99),
        ] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(current, hard_limit, soft_limit_pct),
                expected_pct,
                "expected shedding percentage to be {expected_pct}% at current={current}",
            );
        }

        // At and above hard limit: 100%.
        for current in [hard_limit, hard_limit + 1, 30_000] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(current, hard_limit, soft_limit_pct),
                100,
                "expected 100% shedding at/above hard limit ({current} >= {hard_limit})",
            );
        }

        // Degenerate: soft_limit_pct = 100 acts as a binary cutoff:
        // - below hard_limit: 0%; at/above: 100%.
        for current in [0, hard_limit - 1] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(current, hard_limit, 100),
                0,
                "soft_limit_pct=100: no shedding expected below hard limit ({current} < {hard_limit})",
            );
        }
        for current in [hard_limit, hard_limit + 1] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(current, hard_limit, 100),
                100,
                "soft_limit_pct=100: full shedding expected at/above hard limit ({current} >= \
                    {hard_limit})",
            );
        }

        // Degenerate: soft_limit_pct = 0 means soft_limit = 0; any current > 0 sheds.
        assert_eq!(
            compute_graduated_load_shedding_percentage(0, hard_limit, 0),
            0,
            "soft_limit_pct=0: at current=0, no shedding expected (current <= soft_limit=0)",
        );
        assert_eq!(
            compute_graduated_load_shedding_percentage(hard_limit / 2, hard_limit, 0),
            50,
            "soft_limit_pct=0: at midpoint of hard_limit, 50% shedding expected",
        );
        assert_eq!(
            compute_graduated_load_shedding_percentage(hard_limit, hard_limit, 0),
            100,
            "soft_limit_pct=0: at hard_limit, 100% shedding expected",
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_check_authority_overload() {
        telemetry_subscribers::init_for_testing();

        let config = AuthorityOverloadConfig {
            safe_transaction_ready_rate: 0,
            ..Default::default()
        };
        let state = TestAuthorityBuilder::new()
            .with_authority_overload_config(config.clone())
            .build()
            .await;

        // Initialize latency reporter.
        for _ in 0..1000 {
            state
                .metrics
                .execution_queueing_latency
                .report(Duration::from_secs(20));
        }

        // Creates a simple case to see if authority state overload_info can be updated
        // correctly by check_authority_overload.
        let authority = Arc::downgrade(&state);
        assert!(check_execution_overload(&authority, &config));
        assert!(state.overload_info.is_overload.load(Ordering::Relaxed));
        assert_eq!(
            state
                .overload_info
                .local_load_shedding_percentage
                .load(Ordering::Relaxed),
            config.min_load_shedding_percentage_above_hard_limit
        );

        // Checks that check_authority_overload should return false when the input
        // authority state doesn't exist.
        let authority = Arc::downgrade(&state);
        drop(state);
        assert!(!check_execution_overload(&authority, &config));
    }

    /// Wires `WritebackCacheConfig`'s backpressure thresholds into
    /// `compute_graduated_load_shedding_percentage` and verifies the
    /// post-consensus cache-pressure signal at cache-relevant scales.
    /// Catches regressions in either the config getter defaults or the
    /// call signature used in `check_execution_overload`.
    #[test]
    fn test_writeback_cache_backpressure_soft_limit_pct() {
        use iota_config::node::WritebackCacheConfig;

        // Default getter: 50% with no explicit value or env override.
        let default_config = WritebackCacheConfig::default();
        assert_eq!(default_config.backpressure_soft_limit_pct(), 50);
        assert_eq!(default_config.backpressure_threshold(), 100_000);

        // Explicit config: soft_limit_pct of 75 against a 1000-pending-tx
        // hard limit. Soft limit = 1000 * 75 / 100 = 750.
        let config = WritebackCacheConfig {
            backpressure_threshold: Some(1000),
            backpressure_soft_limit_pct: Some(75),
            ..Default::default()
        };
        assert_eq!(config.backpressure_threshold(), 1000);
        assert_eq!(config.backpressure_soft_limit_pct(), 75);

        // Below soft limit: no shedding.
        for pending in [0u64, 100, 750] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(
                    pending as usize,
                    config.backpressure_threshold() as usize,
                    config.backpressure_soft_limit_pct(),
                ),
                0,
                "no shedding expected at pending={pending} <= soft_limit=750",
            );
        }

        // Halfway between soft (750) and hard (1000): 50% shedding.
        assert_eq!(
            compute_graduated_load_shedding_percentage(
                875,
                config.backpressure_threshold() as usize,
                config.backpressure_soft_limit_pct(),
            ),
            50,
        );

        // At and above hard limit: 100% shedding.
        for pending in [1000u64, 1500, 100_000] {
            assert_eq!(
                compute_graduated_load_shedding_percentage(
                    pending as usize,
                    config.backpressure_threshold() as usize,
                    config.backpressure_soft_limit_pct(),
                ),
                100,
                "100% shedding expected at pending={pending} >= hard_limit=1000",
            );
        }

        // Out-of-range soft_limit_pct is clamped to 100 by the getter.
        let clamped = WritebackCacheConfig {
            backpressure_soft_limit_pct: Some(150),
            ..Default::default()
        };
        assert_eq!(clamped.backpressure_soft_limit_pct(), 100);
    }

    /// Verifies that the cache-pressure signal flows end-to-end from
    /// `WritebackCacheConfig` → `check_execution_overload` →
    /// `overload_info.local_load_shedding_percentage` and the new metric.
    ///
    /// We can't easily inject a non-zero
    /// `approximate_pending_transaction_count` into the test cache without
    /// an additional test hook, so this test instead degenerates the
    /// threshold by setting `backpressure_threshold = 0`, which makes
    /// `compute_graduated_load_shedding_percentage` return 100% even with
    /// `pending_count = 0` (the `current >= hard_limit` branch with `0 >=
    /// 0`). That exercises the full wiring while keeping the test
    /// self-contained.
    #[tokio::test(flavor = "current_thread")]
    async fn test_check_execution_overload_cache_signal_drives_shedding() {
        use iota_config::node::{ExecutionCacheConfig, WritebackCacheConfig};

        telemetry_subscribers::init_for_testing();

        let cache_config = ExecutionCacheConfig {
            writeback_cache: WritebackCacheConfig {
                backpressure_threshold: Some(0),
                ..Default::default()
            },
        };
        let overload_config = AuthorityOverloadConfig::default();
        let state = TestAuthorityBuilder::new()
            .with_authority_overload_config(overload_config.clone())
            .with_cache_config(cache_config)
            .build()
            .await;

        let authority = Arc::downgrade(&state);
        assert!(check_execution_overload(&authority, &overload_config));

        // Cache signal alone should drive 100% shedding via the degenerate
        // threshold; latency and queue signals contribute 0 in a fresh state.
        assert_eq!(
            state
                .metrics
                .cache_backpressure_load_shedding_percentage
                .get(),
            100,
        );
        assert!(state.overload_info.is_overload.load(Ordering::Relaxed));
        assert_eq!(
            state
                .overload_info
                .local_load_shedding_percentage
                .load(Ordering::Relaxed),
            100,
        );
        assert_eq!(state.metrics.authority_load_shedding_percentage.get(), 100,);
    }

    // Creates an AuthorityState and starts an overload monitor that monitors its
    // metrics.
    async fn start_overload_monitor() -> (Arc<AuthorityState>, JoinHandle<()>) {
        let overload_config = AuthorityOverloadConfig::default();
        let state = TestAuthorityBuilder::new()
            .with_authority_overload_config(overload_config.clone())
            .build()
            .await;
        let authority_state = Arc::downgrade(&state);
        let monitor_handle = tokio::spawn(async move {
            overload_monitor(authority_state, overload_config).await;
        });
        (state, monitor_handle)
    }

    // Starts a load generator that generates a steady workload, and also allow it
    // to accept burst of request through `burst_rx`.
    // Request tracking is done by the overload monitor inside `authority`.
    fn start_load_generator(
        steady_rate: f64,
        tx: UnboundedSender<Instant>,
        mut burst_rx: UnboundedReceiver<u32>,
        authority: Arc<AuthorityState>,
        enable_load_shedding: bool,
        total_requests_arc: Arc<AtomicU32>,
        dropped_requests_arc: Arc<AtomicU32>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs_f64(1.0 / steady_rate));
            let mut rng = StdRng::from_rng(&mut OsRng).unwrap();
            let mut total_requests: u32 = 0;
            let mut total_dropped_requests: u32 = 0;

            // Helper function to check whether we should send a request.
            let mut do_send =
                |enable_load_shedding: bool, authority: Arc<AuthorityState>| -> bool {
                    if enable_load_shedding {
                        let shedding_percentage = authority
                            .overload_info
                            .local_load_shedding_percentage
                            .load(Ordering::Relaxed);
                        !(shedding_percentage > 0 && rng.gen_range(0..100) < shedding_percentage)
                    } else {
                        true
                    }
                };

            loop {
                tokio::select! {
                    now = interval.tick() => {
                        total_requests += 1;
                        if do_send(enable_load_shedding, authority.clone()) {
                            if tx.send(now).is_err() {
                                info!("Load generator stopping. Total requests {:?}, total dropped requests {:?}.", total_requests, total_dropped_requests);
                                total_requests_arc.store(total_requests, Ordering::SeqCst);
                                dropped_requests_arc.store(total_dropped_requests, Ordering::SeqCst);
                                return;
                            }
                            authority.metrics.txn_ready_rate_tracker.lock().record();
                        } else {
                            total_dropped_requests += 1;
                        }
                    }
                    Some(burst) = burst_rx.recv() => {
                        let now = Instant::now();
                        total_requests += burst;
                        for _ in 0..burst {
                            if do_send(enable_load_shedding, authority.clone()) {
                                if tx.send(now).is_err() {
                                    info!("Load generator stopping. Total requests {:?}, total dropped requests {:?}.", total_requests, total_dropped_requests);
                                    total_requests_arc.store(total_requests, Ordering::SeqCst);
                                    dropped_requests_arc.store(total_dropped_requests, Ordering::SeqCst);
                                    return;
                                }
                                authority.metrics.txn_ready_rate_tracker.lock().record();
                            } else {
                                total_dropped_requests += 1;
                            }
                        }
                    }
                }
            }
        })
    }

    // Starts a request executor that can consume request based on `execution_rate`.
    // Request tracking is done by the overload monitor inside `authority`.
    fn start_executor(
        execution_rate: f64,
        mut rx: UnboundedReceiver<Instant>,
        mut stop_rx: oneshot::Receiver<()>,
        authority: Arc<AuthorityState>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs_f64(1.0 / execution_rate));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    Some(start_time) = rx.recv() => {
                        authority.metrics.execution_rate_tracker.lock().record();
                        authority.metrics.execution_queueing_latency.report(start_time.elapsed());
                        interval.tick().await;
                    }
                    _ = &mut stop_rx => {
                        info!("Executor stopping");
                        return;
                    }
                }
            }
        })
    }

    // Helper fundtion to periodically print the current overload info.
    async fn sleep_and_print_stats(state: Arc<AuthorityState>, seconds: u32) {
        for _ in 0..seconds {
            info!(
                "Overload: {:?}. Shedding percentage: {:?}. Queue: {:?}, Ready rate: {:?}. Exec rate: {:?}.",
                state.overload_info.is_overload.load(Ordering::Relaxed),
                state
                    .overload_info
                    .local_load_shedding_percentage
                    .load(Ordering::Relaxed),
                state.metrics.execution_queueing_latency.latency(),
                state.metrics.txn_ready_rate_tracker.lock().rate(),
                state.metrics.execution_rate_tracker.lock().rate(),
            );
            sleep(Duration::from_secs(1)).await;
        }
    }

    // Running a workload with consistent steady `generator_rate` and
    // `executor_rate`. It checks that the dropped requests should in between
    // min_dropping_rate and max_dropping_rate.
    async fn run_consistent_workload_test(
        generator_rate: f64,
        executor_rate: f64,
        min_dropping_rate: f64,
        max_dropping_rate: f64,
    ) {
        let (state, monitor_handle) = start_overload_monitor().await;

        let (tx, rx) = unbounded_channel();
        let (_burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            generator_rate,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(executor_rate, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 300).await;

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);

        let dropped_ratio = dropped_requests.load(Ordering::SeqCst) as f64
            / total_requests.load(Ordering::SeqCst) as f64;
        assert!(min_dropping_rate <= dropped_ratio);
        assert!(dropped_ratio <= max_dropping_rate);

        monitor_handle.abort();
        let _ = monitor_handle.await;
    }

    // Tests that when request generation rate is slower than execution rate, no
    // requests should be dropped.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_workload_consistent_no_overload() {
        telemetry_subscribers::init_for_testing();
        run_consistent_workload_test(900.0, 1000.0, 0.0, 0.0).await;
    }

    // Tests that when request generation rate is slightly above execution rate, a
    // small portion of requests should be dropped.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_workload_consistent_slightly_overload() {
        telemetry_subscribers::init_for_testing();
        // Dropping rate should be around 15%.
        run_consistent_workload_test(1100.0, 1000.0, 0.05, 0.25).await;
    }

    // Tests that when request generation rate is much higher than execution rate, a
    // large portion of requests should be dropped.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_workload_consistent_overload() {
        telemetry_subscribers::init_for_testing();
        // Dropping rate should be around 70%.
        run_consistent_workload_test(3000.0, 1000.0, 0.6, 0.8).await;
    }

    // Tests that when there is a very short single spike, no request should be
    // dropped.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_workload_single_spike() {
        telemetry_subscribers::init_for_testing();
        let (state, monitor_handle) = start_overload_monitor().await;

        let (tx, rx) = unbounded_channel();
        let (burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            10.0,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(1000.0, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 10).await;
        // Send out a burst of 5000 requests.
        burst_tx.send(5000).unwrap();
        sleep_and_print_stats(state.clone(), 20).await;

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);

        // No requests should be dropped.
        assert_eq!(dropped_requests.load(Ordering::SeqCst), 0);

        monitor_handle.abort();
        let _ = monitor_handle.await;
    }

    // Tests that when there are regular spikes that keep queueing latency
    // consistently high, overload monitor should kick in and shed load.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_workload_consistent_short_spike() {
        telemetry_subscribers::init_for_testing();
        let (state, monitor_handle) = start_overload_monitor().await;

        let (tx, rx) = unbounded_channel();
        let (burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            10.0,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(1000.0, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 15).await;
        for _ in 0..16 {
            // Regularly send out a burst of request.
            burst_tx.send(10000).unwrap();
            sleep_and_print_stats(state.clone(), 5).await;
        }

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);
        let dropped_ratio = dropped_requests.load(Ordering::SeqCst) as f64
            / total_requests.load(Ordering::SeqCst) as f64;

        // We should drop about 50% of request because the burst throughput is about 2x
        // of execution rate.
        assert!(0.4 < dropped_ratio);
        assert!(dropped_ratio < 0.6);

        monitor_handle.abort();
        let _ = monitor_handle.await;
    }

    // Tests that the ratio of rejected transactions created randomly matches load
    // shedding percentage in the overload monitor.
    #[test]
    fn test_txn_rejection_rate() {
        for rejection_percentage in 0..=100 {
            let mut reject_count = 0;
            for _ in 0..10000 {
                let digest = TransactionDigest::random();
                if should_reject_tx(rejection_percentage, digest, 28455473) {
                    reject_count += 1;
                }
            }

            debug!(
                "Rejection percentage: {:?}, reject count: {:?}.",
                rejection_percentage, reject_count
            );
            // Give it a 3% fluctuation.
            assert!(rejection_percentage as f32 / 100.0 - 0.03 < reject_count as f32 / 10000.0);
            assert!(reject_count as f32 / 10000.0 < rejection_percentage as f32 / 100.0 + 0.03);
        }
    }

    // Tests that rejected transaction will have a chance to be accepted in the
    // future.
    #[sim_test]
    async fn test_txn_rejection_over_time() {
        let start_time = Instant::now();
        let mut digest = TransactionDigest::random();
        let mut temporal_seed = 1708108277 / SEED_UPDATE_DURATION_SECS;
        let load_shedding_percentage = 50;

        // Find a rejected transaction with 50% rejection rate.
        while !should_reject_tx(load_shedding_percentage, digest, temporal_seed)
            && start_time.elapsed() < Duration::from_secs(30)
        {
            digest = TransactionDigest::random();
        }

        // It should always be rejected using the current temporal_seed.
        for _ in 0..100 {
            assert!(should_reject_tx(
                load_shedding_percentage,
                digest,
                temporal_seed
            ));
        }

        // It will be accepted in the future.
        temporal_seed += 1;
        while should_reject_tx(load_shedding_percentage, digest, temporal_seed)
            && start_time.elapsed() < Duration::from_secs(30)
        {
            temporal_seed += 1;
        }

        // Make sure that the tests can finish within 30 seconds.
        assert!(start_time.elapsed() < Duration::from_secs(30));
    }
}
