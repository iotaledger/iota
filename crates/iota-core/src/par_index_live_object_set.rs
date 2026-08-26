// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use iota_sdk_types::ObjectId;
use iota_types::{object::Object, storage::error::Error as StorageError};
use tracing::{info, warn};

use crate::{
    authority::AuthorityStore,
    index_rebuild_cancellation::{RebuildCancelled, is_cancelled},
    progress_logger::PROGRESS_REPORT_INTERVAL,
};

/// Make `LiveObjectIndexer`s for parallel indexing of the live object set
pub trait ParMakeLiveObjectIndexer: Sync {
    type ObjectIndexer<'a>: LiveObjectIndexer
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_>;
}

/// Represents an instance of a indexer that operates on a subset of the live
/// object set
pub trait LiveObjectIndexer {
    /// Called on each object in the range of the live object set this indexer
    /// task is responsible for.
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError>;

    /// Called once the range of objects this indexer task is responsible for
    /// have been processed by calling `index_object`.
    fn finish(self) -> Result<(), StorageError>;
}

/// Utility for iterating over, and indexing, the live object set in parallel
///
/// This is done by dividing the addressable ObjectId space into smaller,
/// disjoint sets and operating on each set in parallel in a separate thread.
/// User's will need to implement the `ParMakeLiveObjectIndexer` trait which
/// will be used to make N `LiveObjectIndexer`s which will then process one of
/// the disjoint parts of the live object set.
///
/// Setting `cancelled` fails the scan early, so a caller that must not wait
/// for a full pass — a shutting-down node, which these threads hold open — can
/// abandon it; the partial work is the caller's to discard.
#[tracing::instrument(skip_all)]
pub fn par_index_live_object_set<T: ParMakeLiveObjectIndexer>(
    authority_store: &AuthorityStore,
    make_indexer: &T,
    cancelled: &AtomicBool,
) -> Result<(), StorageError> {
    info!("Indexing Live Object Set");
    let start_time = Instant::now();

    const BITS: u8 = 5;
    const TASKS: usize = 1 << BITS;

    // Object ids are hash-derived and uniformly distributed, so each task's
    // position within its id range doubles as its fraction of work done.
    let positions: Vec<AtomicU64> = (0..TASKS)
        .map(|index| AtomicU64::new(task_range_start(index as u8, BITS)))
        .collect();
    let objects_scanned = AtomicU64::new(0);
    let done = AtomicBool::new(false);

    std::thread::scope(|s| -> Result<(), StorageError> {
        let reporter = s.spawn(|| {
            report_scan_progress_until_done(BITS, &positions, &objects_scanned, &done, start_time)
        });

        let mut threads = Vec::new();
        for index in 0u8..(TASKS as u8) {
            let position = &positions[index as usize];
            let objects_scanned = &objects_scanned;
            threads.push(s.spawn(move || {
                let object_indexer = make_indexer.make_live_object_indexer();
                live_object_set_index_task(
                    index,
                    BITS,
                    authority_store,
                    object_indexer,
                    position,
                    objects_scanned,
                    cancelled,
                )
            }));
        }

        // Join every task before stopping the reporter, so a failed task
        // can't leave the reporter thread blocking the scope forever.
        let mut result = Ok(());
        for thread in threads {
            if let Err(e) = thread.join().unwrap() {
                result = keep_task_error(result, e);
            }
        }
        done.store(true, Ordering::Relaxed);
        reporter.join().unwrap();

        result
    })?;

    info!(
        "Indexing Live Object Set took {} seconds",
        start_time.elapsed().as_secs()
    );

    Ok(())
}

#[tracing::instrument(skip(authority_store, object_indexer, position, objects_scanned, cancelled))]
fn live_object_set_index_task<T: LiveObjectIndexer>(
    task_id: u8,
    bits: u8,
    authority_store: &AuthorityStore,
    mut object_indexer: T,
    position: &AtomicU64,
    objects_scanned: &AtomicU64,
    cancelled: &AtomicBool,
) -> Result<(), StorageError> {
    const COUNTER_CHUNK: u64 = 10_000;
    const CANCELLATION_CHUNK: u64 = 1_024;

    if cancelled.load(Ordering::Relaxed) {
        return Err(scan_cancelled());
    }

    let mut id_bytes = [0; ObjectId::LENGTH];
    id_bytes[0] = task_id << (8 - bits);
    let start_id = ObjectId::new(id_bytes);

    id_bytes[0] |= (1 << (8 - bits)) - 1;
    for element in id_bytes.iter_mut().skip(1) {
        *element = u8::MAX;
    }
    let end_id = ObjectId::new(id_bytes);

    let mut object_scanned: u64 = 0;
    for live_object in authority_store
        .perpetual_tables
        .range_iter_live_object_set(Some(start_id), Some(end_id))
    {
        let object = live_object?.object;
        object_scanned += 1;
        position.store(id_position(&object.id()), Ordering::Relaxed);
        if object_scanned.is_multiple_of(COUNTER_CHUNK) {
            objects_scanned.fetch_add(COUNTER_CHUNK, Ordering::Relaxed);
        }
        if object_scanned.is_multiple_of(CANCELLATION_CHUNK) && cancelled.load(Ordering::Relaxed) {
            return Err(scan_cancelled());
        }

        object_indexer.index_object(&object)?
    }
    objects_scanned.fetch_add(object_scanned % COUNTER_CHUNK, Ordering::Relaxed);
    position.store(
        task_range_start(task_id, bits) + (task_range_span(bits) - 1),
        Ordering::Relaxed,
    );

    object_indexer.finish()?;

    Ok(())
}

fn scan_cancelled() -> StorageError {
    RebuildCancelled::error("the live object set scan was cancelled")
}

/// The error to report out of the scan, given the one kept so far and the one
/// a further task returned. The discarded error is logged.
///
/// A real failure outranks a cancellation, which every task returns once a
/// shutdown sets the flag: the tasks are joined in a fixed order, so keeping
/// the first error would let a cancellation hide a failure the caller must
/// act on.
fn keep_task_error(
    kept: Result<(), StorageError>,
    error: StorageError,
) -> Result<(), StorageError> {
    match kept {
        Ok(()) => Err(error),
        Err(kept) if is_cancelled(&kept) && !is_cancelled(&error) => Err(error),
        Err(kept) => {
            if !is_cancelled(&error) {
                warn!("another live object set indexing task failed: {error}");
            }
            Err(kept)
        }
    }
}

/// Logs a progress line with estimated percent, scan rate, and remaining time
/// every [`PROGRESS_REPORT_INTERVAL`] until `done` is set.
fn report_scan_progress_until_done(
    bits: u8,
    positions: &[AtomicU64],
    objects_scanned: &AtomicU64,
    done: &AtomicBool,
    start_time: Instant,
) {
    let mut last_report = Instant::now();
    while !done.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
        if last_report.elapsed() < PROGRESS_REPORT_INTERVAL {
            continue;
        }
        last_report = Instant::now();

        let fraction = scan_fraction(bits, positions);
        let scanned = objects_scanned.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed();
        let rate = progress_rate(scanned, elapsed);
        let eta = eta_display(elapsed, fraction);
        info!(
            "Indexing live object set: ~{:.1}% done, {} objects scanned ({} objects/s), ETA ~{eta}",
            fraction * 100.0,
            format_count(scanned),
            format_count(rate as u64),
        );
    }
}

/// The fraction of the object id space the scan tasks have covered so far,
/// capped just below 1 because the estimate is only as exact as the id
/// distribution is uniform.
fn scan_fraction(bits: u8, positions: &[AtomicU64]) -> f64 {
    let span = task_range_span(bits) as f64;
    let sum: f64 = positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let start = task_range_start(index as u8, bits);
            let position = position.load(Ordering::Relaxed);
            ((position.saturating_sub(start)) as f64 / span).clamp(0.0, 1.0)
        })
        .sum();
    (sum / positions.len() as f64).min(0.999)
}

/// The first 8 bytes of `task_id`'s range of the object id space.
fn task_range_start(task_id: u8, bits: u8) -> u64 {
    (task_id as u64) << (64 - bits)
}

/// The size of each task's range of the object id space, in units of
/// [`id_position`].
fn task_range_span(bits: u8) -> u64 {
    1u64 << (64 - bits)
}

/// An object id's position in the id space: its first 8 bytes as an integer.
fn id_position(id: &ObjectId) -> u64 {
    id.as_bytes()
        .iter()
        .take(8)
        .fold(0u64, |acc, byte| (acc << 8) | *byte as u64)
}

/// Extrapolates how much longer the work will take, assuming the rate so far
/// holds. Returns `None` when no progress was made yet.
fn estimated_time_remaining(elapsed: Duration, fraction_done: f64) -> Option<Duration> {
    if fraction_done <= 0.0 || fraction_done > 1.0 {
        return None;
    }
    Duration::try_from_secs_f64(elapsed.as_secs_f64() * (1.0 - fraction_done) / fraction_done).ok()
}

/// Items processed per second since the work started.
pub(crate) fn progress_rate(items: u64, elapsed: Duration) -> f64 {
    items as f64 / elapsed.as_secs_f64().max(f64::EPSILON)
}

/// The estimated time remaining formatted for progress lines, or "unknown"
/// when no progress was made yet.
pub(crate) fn eta_display(elapsed: Duration, fraction_done: f64) -> String {
    estimated_time_remaining(elapsed, fraction_done)
        .map(format_duration)
        .unwrap_or_else(|| "unknown".to_string())
}

/// Formats a duration for progress lines, e.g. "1h 42m", "3m 20s", or "45s".
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Formats a count for progress lines, e.g. "123.4M" or "1.2k".
fn format_count(count: u64) -> String {
    if count >= 1_000_000_000 {
        format!("{:.1}B", count as f64 / 1e9)
    } else if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1e6)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1e3)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BITS: u8 = 5;

    fn positions_at(fractions: &[f64]) -> Vec<AtomicU64> {
        fractions
            .iter()
            .enumerate()
            .map(|(index, fraction)| {
                let start = task_range_start(index as u8, BITS);
                let offset = (task_range_span(BITS) as f64 * fraction) as u64;
                AtomicU64::new(start.saturating_add(offset))
            })
            .collect()
    }

    /// A shutdown cancels every task, so whichever task hit a real failure
    /// must still be the error the scan reports, whatever its position in the
    /// join order.
    #[test]
    fn a_real_failure_outranks_a_cancellation_in_any_join_order() {
        let failure = || StorageError::custom("object 0x3 is corrupt");

        let kept = keep_task_error(keep_task_error(Ok(()), scan_cancelled()), failure());
        let error = kept.unwrap_err();
        assert!(!is_cancelled(&error));
        assert!(error.to_string().contains("corrupt"));

        let kept = keep_task_error(keep_task_error(Ok(()), failure()), scan_cancelled());
        let error = kept.unwrap_err();
        assert!(!is_cancelled(&error));
        assert!(error.to_string().contains("corrupt"));

        // Between real failures the first one still wins.
        let kept = keep_task_error(
            keep_task_error(Ok(()), StorageError::custom("first")),
            StorageError::custom("second"),
        );
        assert!(kept.unwrap_err().to_string().contains("first"));

        // Without a real failure the scan still reports the cancellation.
        let kept = keep_task_error(keep_task_error(Ok(()), scan_cancelled()), scan_cancelled());
        assert!(is_cancelled(&kept.unwrap_err()));
    }

    #[test]
    fn scan_fraction_averages_task_positions() {
        let positions = positions_at(&[0.0; 32]);
        assert_eq!(scan_fraction(BITS, &positions), 0.0);

        let positions = positions_at(&[0.5; 32]);
        assert!((scan_fraction(BITS, &positions) - 0.5).abs() < 1e-9);

        let mut fractions = [0.0; 32];
        fractions[0] = 1.0;
        let positions = positions_at(&fractions);
        assert!((scan_fraction(BITS, &positions) - 1.0 / 32.0).abs() < 1e-9);
    }

    #[test]
    fn scan_fraction_is_capped_below_one() {
        let positions = positions_at(&[1.0; 32]);
        assert_eq!(scan_fraction(BITS, &positions), 0.999);
    }

    #[test]
    fn id_position_uses_the_first_eight_bytes() {
        let mut bytes = [0u8; ObjectId::LENGTH];
        bytes[..8].copy_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]);
        bytes[8] = 0xff;
        assert_eq!(id_position(&ObjectId::new(bytes)), 0x1234_5678_9abc_def0);
    }

    #[test]
    fn estimated_time_remaining_extrapolates_the_average_rate() {
        let eta = estimated_time_remaining(Duration::from_secs(100), 0.25).unwrap();
        assert_eq!(eta, Duration::from_secs(300));

        assert_eq!(
            estimated_time_remaining(Duration::from_secs(100), 1.0),
            Some(Duration::ZERO)
        );
        assert_eq!(
            estimated_time_remaining(Duration::from_secs(100), 0.0),
            None
        );
        assert_eq!(
            estimated_time_remaining(Duration::from_secs(100), -0.5),
            None
        );
    }

    #[test]
    fn format_duration_picks_the_two_largest_units() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(200)), "3m 20s");
        assert_eq!(format_duration(Duration::from_secs(6120)), "1h 42m");
    }

    #[test]
    fn format_count_abbreviates_large_numbers() {
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_200), "1.2k");
        assert_eq!(format_count(123_400_000), "123.4M");
        assert_eq!(format_count(2_500_000_000), "2.5B");
    }
}
