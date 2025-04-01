// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};

/// File name of the mainnet unlock data.
pub const INPUT_FILE: &str = "aggregated_mainnet_unlocks.csv";

/// Represents a single entry in the store.
/// It defines how many tokens still remain locked at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StillLockedEntry {
    /// UTC timestamp at which the tokens are still locked.
    pub timestamp: DateTime<Utc>,
    /// Total locked amount (nano-units) still locked at the timestamp.
    pub amount_still_locked: u64,
}

/// In-memory store holding the aggregated token unlock data.
#[derive(Debug, Clone)]
pub struct MainnetUnlocksStore {
    // Each entry represents the total number of tokens still locked at the specific point in time.
    entries: BTreeMap<DateTime<Utc>, StillLockedEntry>,
}

impl MainnetUnlocksStore {
    /// Creates a new store with the aggregated unlock data for mainnet.
    /// Loads the aggregated token unlock data from the given JSON file at the
    /// crate root.
    pub fn new() -> Result<Self> {
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(crate_dir).join("data").join(INPUT_FILE);

        let data = fs::read_to_string(&path)
            .with_context(|| format!("could not read locked supply file: {:?}", path))?;

        Self::from_csv_str(&data)
    }

    /// Parses the given CSV string into a `MainnetUnlocksStore`.
    fn from_csv_str(csv_str: &str) -> Result<Self> {
        let mut rdr = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(csv_str.as_bytes());

        let mut map = BTreeMap::new();

        for result in rdr.deserialize() {
            let entry: StillLockedEntry =
                result.context("failed to deserialize CSV row into StillLockedEntry")?;

            if let Some(old_entry) = map.insert(entry.timestamp, entry) {
                return Err(anyhow::anyhow!(
                    "duplicate entry found for timestamp: {}",
                    old_entry.timestamp
                ));
            }
        }

        Ok(Self { entries: map })
    }

    /// Returns the total amount of tokens (in nano-units) that are still locked
    /// at the given timestamp.
    pub fn still_locked_tokens(&self, date_time: DateTime<Utc>) -> u64 {
        self.entries
            .range(..=date_time)
            .next_back()
            .map(|(_, entry)| entry.amount_still_locked)
            .unwrap_or_else(|| {
                // No earlier entries exist: use first available as retroactively valid
                self.entries
                    .iter()
                    .next()
                    .map(|(_, e)| e.amount_still_locked)
                    .unwrap_or(0)
            })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn store(csv: &str) -> MainnetUnlocksStore {
        MainnetUnlocksStore::from_csv_str(csv).unwrap()
    }

    #[test]
    fn test_no_entries() {
        let store = store("timestamp,amount_still_locked\n");
        assert_eq!(store.still_locked_tokens(Utc::now()), 0);
    }

    #[test]
    fn test_single_entry() {
        let csv = r#"
            timestamp,amount_still_locked
            2000-01-01T00:00:00Z,999
        "#;
        let store = store(csv.trim());

        let before = Utc.with_ymd_and_hms(1999, 12, 31, 23, 59, 59).unwrap();
        let exact = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2001, 1, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(before), 999);
        assert_eq!(store.still_locked_tokens(exact), 999);
        assert_eq!(store.still_locked_tokens(after), 999);
    }

    #[test]
    fn test_multiple_entries() {
        let csv = r#"
            timestamp,amount_still_locked
            2023-01-01T00:00:00Z,300
            2024-01-01T00:00:00Z,200
            2025-01-01T00:00:00Z,100
        "#;
        let store = store(csv.trim());

        let t0 = Utc.with_ymd_and_hms(2022, 12, 31, 0, 0, 0).unwrap();
        let t0_between = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let t4 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(t0), 300);
        assert_eq!(store.still_locked_tokens(t1), 300);
        assert_eq!(store.still_locked_tokens(t0_between), 300);
        assert_eq!(store.still_locked_tokens(t2), 200);
        assert_eq!(store.still_locked_tokens(t3), 100);
        assert_eq!(store.still_locked_tokens(t4), 100);
    }

    #[test]
    fn test_zero_at_latest_entry() {
        let csv = r#"
            timestamp,amount_still_locked
            2023-01-01T00:00:00Z,1000
            2025-01-01T00:00:00Z,0
        "#;
        let store = store(csv.trim());

        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(t1), 1000);
        assert_eq!(store.still_locked_tokens(t2), 0);
        assert_eq!(store.still_locked_tokens(t3), 0);
    }

    #[test]
    fn test_gap_between_entries() {
        let csv = r#"
            timestamp,amount_still_locked
            2020-01-01T00:00:00Z,1000
            2030-01-01T00:00:00Z,100
        "#;
        let store = store(csv.trim());

        let t_before = Utc.with_ymd_and_hms(2019, 1, 1, 0, 0, 0).unwrap();
        let t_mid = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let t_after = Utc.with_ymd_and_hms(2040, 1, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(t_before), 1000);
        assert_eq!(store.still_locked_tokens(t_mid), 1000);
        assert_eq!(store.still_locked_tokens(t_after), 100);
    }

    #[test]
    fn test_dense_entry() {
        let csv = r#"
            timestamp,amount_still_locked
            2023-10-01T00:00:00Z,300
            2023-10-15T00:00:00Z,200
            2023-11-01T00:00:00Z,100
        "#;
        let store = store(csv.trim());

        let t_exact_mid = Utc.with_ymd_and_hms(2023, 10, 15, 0, 0, 0).unwrap();
        let t_between = Utc.with_ymd_and_hms(2023, 10, 20, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(t_exact_mid), 200);
        assert_eq!(store.still_locked_tokens(t_between), 200);
    }

    #[test]
    fn test_first_entry_is_retrospective() {
        let csv = r#"
            timestamp,amount_still_locked
            2022-06-01T00:00:00Z,888
        "#;
        let store = store(csv.trim());

        let far_before = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let just_before = Utc.with_ymd_and_hms(2022, 5, 31, 23, 59, 59).unwrap();
        let exact = Utc.with_ymd_and_hms(2022, 6, 1, 0, 0, 0).unwrap();
        let just_after = Utc.with_ymd_and_hms(2022, 6, 1, 0, 0, 1).unwrap();

        assert_eq!(store.still_locked_tokens(far_before), 888);
        assert_eq!(store.still_locked_tokens(just_before), 888);
        assert_eq!(store.still_locked_tokens(exact), 888);
        assert_eq!(store.still_locked_tokens(just_after), 888);
    }

    #[test]
    fn test_unsorted_input() {
        let csv = r#"
            timestamp,amount_still_locked
            2023-11-01T00:00:00Z,100
            2023-01-01T00:00:00Z,300
            2023-10-01T00:00:00Z,200
        "#;
        let store = store(csv.trim());

        let query_before = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(query_before), 300);

        let query_exact = Utc.with_ymd_and_hms(2023, 10, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(query_exact), 200);
    }
}
