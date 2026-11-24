// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Progress reporting for the long passes a node makes over a whole table.

use std::time::Duration;

/// How often a pass writes a progress line.
pub(crate) const PROGRESS_REPORT_INTERVAL: Duration = Duration::from_secs(1);

/// Cap on a reported time left, so a stalled pass cannot overflow `Duration`.
const LONGEST_REPORTED_WAIT: f64 = u32::MAX as f64;

/// Formats one progress line, e.g. `"<pass>: ~2.9% done, 26.3M/~80.3M objects
/// scanned (4.4M objects/s), ETA ~3m 23s"`.
///
/// `fraction_done` is taken separately from `done / total` so a pass can report
/// a more accurate measure of its position, such as its place in the object id
/// space.
pub(crate) fn progress_line(
    pass: &str,
    unit: &str,
    fraction_done: f64,
    done: u64,
    total: u64,
    elapsed: Duration,
) -> String {
    let rate = format_count(progress_rate(done, elapsed) as u64);
    // `total` comes from RocksDB's key estimate, which can under-report a
    // heavily pruned table by any margin. Once the scan has passed it, it
    // says nothing about what is left.
    if total <= done {
        return format!(
            "{pass}: {} {unit} scanned ({rate} {unit}/s), total unknown",
            format_count(done),
        );
    }
    format!(
        "{pass}: ~{:.1}% done, {}/~{} {unit} scanned ({rate} {unit}/s), ETA ~{}",
        fraction_done * 100.0,
        format_count(done),
        format_count(total),
        eta_display(elapsed, fraction_done),
    )
}

/// Items processed per second since the work started.
pub(crate) fn progress_rate(items: u64, elapsed: Duration) -> f64 {
    items as f64 / elapsed.as_secs_f64().max(f64::EPSILON)
}

/// The estimated time remaining, or "unknown" when nothing has been done yet.
pub(crate) fn eta_display(elapsed: Duration, fraction_done: f64) -> String {
    if fraction_done <= 0.0 {
        return "unknown".to_string();
    }
    let total = elapsed.as_secs_f64() / fraction_done;
    let left = (total - elapsed.as_secs_f64()).clamp(0.0, LONGEST_REPORTED_WAIT);
    format_duration(Duration::from_secs_f64(left))
}

/// Formats a duration for progress lines, e.g. "1h 42m", "3m 20s", or "45s".
pub(crate) fn format_duration(duration: Duration) -> String {
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
pub(crate) fn format_count(count: u64) -> String {
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

    /// Pins the format of a progress line, which every pass shares.
    #[test]
    fn a_progress_line_reads_the_same_for_every_pass() {
        assert_eq!(
            progress_line(
                "Indexing live object set",
                "objects",
                0.029,
                26_300_000,
                803_000_000,
                Duration::from_secs(210),
            ),
            "Indexing live object set: ~2.9% done, 26.3M/~803.0M objects scanned \
             (125.2k objects/s), ETA ~1h 57m"
        );
    }

    /// A total of `0` (no estimate) is reported as unknown, with no percentage
    /// or ETA.
    #[test]
    fn an_unknown_total_is_not_reported_as_complete() {
        let line = progress_line(
            "Backfilling history",
            "transactions",
            0.0,
            9_100_000,
            0,
            Duration::from_secs(125),
        );
        assert_eq!(
            line,
            "Backfilling history: 9.1M transactions scanned (72.8k transactions/s), \
             total unknown"
        );
        assert!(!line.contains('%'), "{line}");
        assert!(!line.contains("ETA"), "{line}");

        // So is an estimate the scan has overtaken.
        let line = progress_line(
            "Backfilling history",
            "effects",
            1.0,
            104_000_000,
            193_200,
            Duration::from_secs(1432),
        );
        assert!(!line.contains('%'), "{line}");
        assert!(!line.contains("ETA"), "{line}");
        assert!(line.contains("104.0M effects scanned"), "{line}");
    }

    /// Nothing done yet says so rather than extrapolating from no data.
    #[test]
    fn an_eta_needs_progress_to_estimate_from() {
        assert_eq!(eta_display(Duration::from_secs(100), 0.0), "unknown");
        assert_eq!(eta_display(Duration::from_secs(100), 0.25), "5m 0s");
        assert_eq!(eta_display(Duration::from_secs(100), 1.0), "0s");
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
