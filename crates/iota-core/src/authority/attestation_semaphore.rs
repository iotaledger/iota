// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Limits how many attestation dry runs a validator runs at once, how long a
//! submission may wait for one, and how many may wait.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use iota_config::node::AuthorityOverloadConfig;
use iota_types::error::{IotaError, IotaResult};
use prometheus_filtered::{Histogram, IntCounter, IntGauge};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{authority::AuthorityMetrics, runtime::available_cpu_cores};

/// The retry hint of a rejection. It is longer than the default
/// [`AuthorityOverloadConfig::max_attestation_wait`] (500 ms), so by the time
/// a client retries, every submission that was waiting has got a dry run or
/// been rejected.
const RETRY_AFTER_SECS: u64 = 1;

/// The number of submissions that may wait when
/// [`AuthorityOverloadConfig::max_queued_attestations`] is unset.
const DEFAULT_MAX_QUEUED_ATTESTATIONS: usize = 1_024;

/// The longest wait for a dry run when
/// [`AuthorityOverloadConfig::max_attestation_wait`] is unset.
const DEFAULT_MAX_ATTESTATION_WAIT: Duration = Duration::from_millis(500);

/// Each new dry run time moves the average toward it by the difference
/// divided by this.
const AVERAGE_DRY_RUN_TIME_DIVISOR: u64 = 8;

/// The attestation dry runs a validator may run at once, with a queue of
/// submissions waiting for one, bounded in time and in number.
pub(crate) struct AttestationSemaphore {
    permits: Arc<Semaphore>,
    max_concurrent: usize,
    max_waiting: usize,
    max_wait: Duration,
    waiting: AtomicUsize,
    /// Running average of how long a permit is held, in microseconds; zero
    /// until the first permit is released.
    average_dry_run_micros: Arc<AtomicU64>,
    in_progress_gauge: IntGauge,
    waiting_gauge: IntGauge,
    rejected_counter: IntCounter,
    wait_histogram: Histogram,
}

/// Allows one attestation dry run. Dropping it frees the place for the next
/// one.
pub(crate) struct AttestationPermit {
    _permit: OwnedSemaphorePermit,
    acquired: Instant,
    average_dry_run_micros: Arc<AtomicU64>,
    in_progress_gauge: IntGauge,
}

impl Drop for AttestationPermit {
    fn drop(&mut self) {
        record_dry_run_time(&self.average_dry_run_micros, self.acquired.elapsed());
        self.in_progress_gauge.dec();
    }
}

/// Moves the running average in `average_micros` toward `time`.
fn record_dry_run_time(average_micros: &AtomicU64, time: Duration) {
    let sample = u64::try_from(time.as_micros()).unwrap_or(u64::MAX);
    average_micros.update(Ordering::Relaxed, Ordering::Relaxed, |average| {
        if average == 0 {
            sample
        } else {
            average - average / AVERAGE_DRY_RUN_TIME_DIVISOR + sample / AVERAGE_DRY_RUN_TIME_DIVISOR
        }
    });
}

/// A submission's place in the queue. Dropping it gives the place up, also
/// when the wait is cancelled.
struct QueuePlace<'a> {
    semaphore: &'a AttestationSemaphore,
}

impl Drop for QueuePlace<'_> {
    fn drop(&mut self) {
        self.semaphore.waiting.fetch_sub(1, Ordering::AcqRel);
        self.semaphore.waiting_gauge.dec();
    }
}

impl AttestationSemaphore {
    /// Returns a semaphore sized by
    /// [`AuthorityOverloadConfig::max_concurrent_attestations`],
    /// [`AuthorityOverloadConfig::max_queued_attestations`] and
    /// [`AuthorityOverloadConfig::max_attestation_wait`] of `config`, with the
    /// defaults their docs give.
    pub(crate) fn new(config: &AuthorityOverloadConfig, metrics: &AuthorityMetrics) -> Self {
        let max_concurrent = config
            .max_concurrent_attestations
            .unwrap_or_else(|| available_cpu_cores() / 4)
            .clamp(1, Semaphore::MAX_PERMITS);

        Self {
            permits: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
            max_waiting: config
                .max_queued_attestations
                .unwrap_or(DEFAULT_MAX_QUEUED_ATTESTATIONS),
            max_wait: config
                .max_attestation_wait
                .unwrap_or(DEFAULT_MAX_ATTESTATION_WAIT),
            waiting: AtomicUsize::new(0),
            average_dry_run_micros: Arc::new(AtomicU64::new(0)),
            in_progress_gauge: metrics.attestations_in_progress.clone(),
            waiting_gauge: metrics.attestations_waiting.clone(),
            rejected_counter: metrics.attestations_rejected_total.clone(),
            wait_histogram: metrics.attestation_permit_wait.clone(),
        }
    }

    /// Returns a permit at once when one is free, and otherwise waits for one
    /// while the queue has room, for at most
    /// [`AuthorityOverloadConfig::max_attestation_wait`].
    ///
    /// # Errors
    ///
    /// [`IotaError::ValidatorOverloadedRetryAfter`] when every permit is taken
    /// and the queue is full, or when no permit frees within
    /// [`AuthorityOverloadConfig::max_attestation_wait`].
    ///
    /// # Panics
    ///
    /// If the semaphore is closed. Nothing closes it.
    pub(crate) async fn acquire(&self) -> IotaResult<AttestationPermit> {
        let arrived = Instant::now();
        let permit = match self.permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let _place = self.join_queue()?;

                tokio::time::timeout(self.max_wait, self.permits.clone().acquire_owned())
                    .await
                    .map_err(|_| self.reject())?
                    // Nothing closes the semaphore.
                    .expect("the attestation semaphore should not be closed")
            }
        };
        self.wait_histogram.observe(arrived.elapsed().as_secs_f64());
        self.in_progress_gauge.inc();

        Ok(AttestationPermit {
            _permit: permit,
            acquired: Instant::now(),
            average_dry_run_micros: self.average_dry_run_micros.clone(),
            in_progress_gauge: self.in_progress_gauge.clone(),
        })
    }

    /// Fails with [`IotaError::ValidatorOverloadedRetryAfter`] when every
    /// permit is taken, and either the queue is full or the expected wait is
    /// longer than [`AuthorityOverloadConfig::max_attestation_wait`]. Takes no
    /// permit and no place in the queue, so a submission can be refused
    /// before any work is done for it.
    pub(crate) fn check_overload(&self) -> IotaResult {
        if self.permits.available_permits() > 0 {
            return Ok(());
        }
        let waiting = self.waiting.load(Ordering::Acquire);
        if waiting >= self.max_waiting || self.expected_wait(waiting) > self.max_wait {
            return Err(self.reject());
        }

        Ok(())
    }

    /// The wait a submission arriving behind `waiting` others can expect while
    /// every permit is taken: one average dry run time for each of them and for
    /// itself, divided by the number of permits.
    fn expected_wait(&self, waiting: usize) -> Duration {
        let average = self.average_dry_run_micros.load(Ordering::Relaxed);
        let queued = u64::try_from(waiting).unwrap_or(u64::MAX).saturating_add(1);
        let permits = u64::try_from(self.max_concurrent).unwrap_or(u64::MAX);

        Duration::from_micros(queued.saturating_mul(average) / permits)
    }

    fn join_queue(&self) -> IotaResult<QueuePlace<'_>> {
        self.waiting
            .try_update(Ordering::AcqRel, Ordering::Acquire, |waiting| {
                (waiting < self.max_waiting).then_some(waiting + 1)
            })
            .map_err(|_| self.reject())?;
        self.waiting_gauge.inc();

        Ok(QueuePlace { semaphore: self })
    }

    fn reject(&self) -> IotaError {
        self.rejected_counter.inc();

        IotaError::ValidatorOverloadedRetryAfter {
            retry_after_secs: RETRY_AFTER_SECS,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use prometheus_filtered::Registry;

    use super::*;

    fn semaphore(
        max_concurrent: Option<usize>,
        max_queued: Option<usize>,
    ) -> (Arc<AttestationSemaphore>, AuthorityMetrics) {
        semaphore_with_wait(max_concurrent, max_queued, Duration::from_secs(30))
    }

    fn semaphore_with_wait(
        max_concurrent: Option<usize>,
        max_queued: Option<usize>,
        max_wait: Duration,
    ) -> (Arc<AttestationSemaphore>, AuthorityMetrics) {
        let config = AuthorityOverloadConfig {
            max_concurrent_attestations: max_concurrent,
            max_queued_attestations: max_queued,
            max_attestation_wait: Some(max_wait),
            ..Default::default()
        };
        let metrics = AuthorityMetrics::new(&Registry::new());

        (
            Arc::new(AttestationSemaphore::new(&config, &metrics)),
            metrics,
        )
    }

    fn is_overload_rejection(result: &IotaResult<impl Sized>) -> bool {
        matches!(
            result,
            Err(IotaError::ValidatorOverloadedRetryAfter {
                retry_after_secs: RETRY_AFTER_SECS
            })
        )
    }

    async fn wait_until_queued(semaphore: &AttestationSemaphore, count: usize) {
        tokio::time::timeout(Duration::from_secs(30), async {
            while semaphore.waiting.load(Ordering::Acquire) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the submissions should be queued");
    }

    #[test]
    fn unset_limits_follow_the_available_cores() {
        let metrics = AuthorityMetrics::new(&Registry::new());
        let semaphore = AttestationSemaphore::new(&AuthorityOverloadConfig::default(), &metrics);

        let expected = (available_cpu_cores() / 4).max(1);
        assert_eq!(semaphore.permits.available_permits(), expected);
        assert_eq!(semaphore.max_waiting, DEFAULT_MAX_QUEUED_ATTESTATIONS);
        assert_eq!(semaphore.max_wait, DEFAULT_MAX_ATTESTATION_WAIT);
    }

    #[test]
    fn a_limit_of_zero_dry_runs_still_allows_one() {
        let (semaphore, _metrics) = semaphore(Some(0), None);

        assert_eq!(semaphore.permits.available_permits(), 1);
        assert_eq!(semaphore.max_waiting, DEFAULT_MAX_QUEUED_ATTESTATIONS);
    }

    #[test]
    fn the_first_dry_run_time_sets_the_average_and_later_ones_move_it() {
        let average_micros = AtomicU64::new(0);

        record_dry_run_time(&average_micros, Duration::from_millis(80));
        assert_eq!(average_micros.load(Ordering::Relaxed), 80_000);

        record_dry_run_time(&average_micros, Duration::ZERO);
        assert_eq!(average_micros.load(Ordering::Relaxed), 70_000);
    }

    #[tokio::test]
    async fn a_submission_that_waits_too_long_is_refused() {
        let (semaphore, metrics) = semaphore_with_wait(Some(1), Some(8), Duration::from_millis(20));
        let _held = semaphore.acquire().await.unwrap();

        assert!(is_overload_rejection(&semaphore.acquire().await));
        assert_eq!(semaphore.waiting.load(Ordering::Acquire), 0);
        assert_eq!(metrics.attestations_waiting.get(), 0);
        assert_eq!(metrics.attestations_rejected_total.get(), 1);
    }

    #[tokio::test]
    async fn a_long_expected_wait_is_refused_before_any_work() {
        let (semaphore, metrics) =
            semaphore_with_wait(Some(1), Some(8), Duration::from_millis(100));
        let _held = semaphore.acquire().await.unwrap();

        // One average dry run time ahead of a submission arriving now.
        semaphore
            .average_dry_run_micros
            .store(50_000, Ordering::Relaxed);
        semaphore.check_overload().unwrap();

        semaphore
            .average_dry_run_micros
            .store(200_000, Ordering::Relaxed);
        assert!(is_overload_rejection(&semaphore.check_overload()));
        assert_eq!(metrics.attestations_rejected_total.get(), 1);
    }

    #[tokio::test]
    async fn no_more_dry_runs_than_the_limit_run_at_once() {
        let (semaphore, metrics) = semaphore(Some(2), Some(64));
        let running = Arc::new(AtomicUsize::new(0));
        let most_running = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..16)
            .map(|_| {
                let semaphore = semaphore.clone();
                let running = running.clone();
                let most_running = most_running.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    let now_running = running.fetch_add(1, Ordering::SeqCst) + 1;
                    most_running.fetch_max(now_running, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(most_running.load(Ordering::SeqCst), 2);
        assert_eq!(metrics.attestations_in_progress.get(), 0);
        assert_eq!(metrics.attestations_waiting.get(), 0);
        assert_eq!(metrics.attestations_rejected_total.get(), 0);
    }

    #[tokio::test]
    async fn a_full_queue_rejects_until_a_permit_frees() {
        let (semaphore, metrics) = semaphore(Some(1), Some(1));
        let held = semaphore.acquire().await.unwrap();
        let waiter = tokio::spawn({
            let semaphore = semaphore.clone();

            async move { semaphore.acquire().await.map(drop) }
        });
        wait_until_queued(&semaphore, 1).await;
        assert_eq!(metrics.attestations_in_progress.get(), 1);
        assert_eq!(metrics.attestations_waiting.get(), 1);

        assert!(is_overload_rejection(&semaphore.acquire().await));
        assert!(is_overload_rejection(&semaphore.check_overload()));
        assert_eq!(metrics.attestations_rejected_total.get(), 2);

        drop(held);
        waiter.await.unwrap().unwrap();
        semaphore.check_overload().unwrap();
        assert_eq!(metrics.attestations_in_progress.get(), 0);
        assert_eq!(metrics.attestations_waiting.get(), 0);
    }

    #[tokio::test]
    async fn without_a_queue_a_busy_semaphore_rejects_at_once() {
        let (semaphore, metrics) = semaphore(Some(1), Some(0));
        semaphore.check_overload().unwrap();

        let held = semaphore.acquire().await.unwrap();
        assert!(is_overload_rejection(&semaphore.check_overload()));
        assert!(is_overload_rejection(&semaphore.acquire().await));
        assert_eq!(metrics.attestations_rejected_total.get(), 2);

        drop(held);
        semaphore.check_overload().unwrap();
    }

    #[tokio::test]
    async fn a_cancelled_wait_gives_up_its_place() {
        let (semaphore, metrics) = semaphore(Some(1), Some(1));
        let _held = semaphore.acquire().await.unwrap();

        // Timing out means the call was waiting; a rejection returns at once
        // with an error.
        for _ in 0..2 {
            let wait = tokio::time::timeout(Duration::from_millis(10), semaphore.acquire()).await;
            assert!(wait.is_err(), "the submission should have been queued");
            assert_eq!(metrics.attestations_waiting.get(), 0);
            assert_eq!(semaphore.waiting.load(Ordering::Acquire), 0);
        }
        assert_eq!(metrics.attestations_rejected_total.get(), 0);
    }

    #[tokio::test]
    async fn a_dry_run_that_panics_frees_its_permit() {
        let (semaphore, metrics) = semaphore(Some(1), Some(0));
        let permit = semaphore.acquire().await.unwrap();

        let join_error = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            panic!("the dry run panicked");
        })
        .await
        .unwrap_err();

        assert!(join_error.is_panic());
        assert_eq!(metrics.attestations_in_progress.get(), 0);
        semaphore.check_overload().unwrap();
        drop(semaphore.acquire().await.unwrap());
    }
}
