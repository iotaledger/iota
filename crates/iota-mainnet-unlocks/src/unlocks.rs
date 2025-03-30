// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// File name of the mainnet unlock data.
const AGGREGATED_MAINNET_UNLOCKS_FILE: &str = "mainnet_unlocks_aggregated.json";

/// Represents a single entry in the aggregated unlock data.
/// It defines how many tokens still remain locked at a specific point in time.
#[derive(Debug, Clone, Deserialize)]
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
        let path = PathBuf::from(crate_dir)
            .join("data")
            .join(AGGREGATED_MAINNET_UNLOCKS_FILE);

        let data = fs::read_to_string(&path)
            .with_context(|| format!("could not read locked supply file: {:?}", path))?;

        Self::from_json(&data)
    }

    /// Parses the given JSON string into a `MainnetUnlocksStore`.
    fn from_json(json: &str) -> Result<Self> {
        let parsed: Vec<StillLockedEntry> =
            serde_json::from_str(json).context("invalid JSON format in unlock data")?;

        let mut map = BTreeMap::new();
        for entry in parsed {
            if map.contains_key(&entry.timestamp) {
                return Err(anyhow::anyhow!(
                    "duplicate entry found for timestamp: {}",
                    entry.timestamp
                ));
            }
            map.insert(entry.timestamp, entry);
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

    #[test]
    fn test_no_entries() {
        let store = MainnetUnlocksStore::from_json("[]").unwrap();
        assert_eq!(store.still_locked_tokens(Utc::now()), 0);
    }

    #[test]
    fn test_single_entry() {
        let json = r#"
            [
                { "timestamp": "2000-01-01T00:00:00Z", "amount_still_locked": 999 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let before = Utc.with_ymd_and_hms(1999, 12, 31, 23, 59, 59).unwrap();
        let exact = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2001, 1, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(before), 999);
        assert_eq!(store.still_locked_tokens(exact), 999);
        assert_eq!(store.still_locked_tokens(after), 999);
    }

    #[test]
    fn test_multiple_entries() {
        let json = r#"
            [
                { "timestamp": "2023-01-01T00:00:00Z", "amount_still_locked": 300 },
                { "timestamp": "2024-01-01T00:00:00Z", "amount_still_locked": 200 },
                { "timestamp": "2025-01-01T00:00:00Z", "amount_still_locked": 100 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let t0 = Utc.with_ymd_and_hms(2022, 12, 31, 0, 0, 0).unwrap(); // before all
        let t0_between = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap(); // between entry 1 and 2
        let t1 = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(); // first entry
        let t2 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(); // second entry
        let t3 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(); // third entry
        let t4 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(); // after all

        assert_eq!(store.still_locked_tokens(t0), 300); // first entry is retroactively valid
        assert_eq!(store.still_locked_tokens(t1), 300);
        assert_eq!(store.still_locked_tokens(t0_between), 300);
        assert_eq!(store.still_locked_tokens(t2), 200);
        assert_eq!(store.still_locked_tokens(t3), 100);
        assert_eq!(store.still_locked_tokens(t4), 100); // last entry remains valid
    }

    #[test]
    fn test_zero_at_latest_entry() {
        let json = r#"
            [
                { "timestamp": "2023-01-01T00:00:00Z", "amount_still_locked": 1000 },
                { "timestamp": "2025-01-01T00:00:00Z", "amount_still_locked": 0 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(); // between entries
        let t2 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(); // entry with zero
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(); // after all

        assert_eq!(store.still_locked_tokens(t1), 1000);
        assert_eq!(store.still_locked_tokens(t2), 0);
        assert_eq!(store.still_locked_tokens(t3), 0);
    }

    #[test]
    fn test_gap_between_entries() {
        let json = r#"
            [
                { "timestamp": "2020-01-01T00:00:00Z", "amount_still_locked": 1000 },
                { "timestamp": "2030-01-01T00:00:00Z", "amount_still_locked": 100 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let t_before = Utc.with_ymd_and_hms(2019, 1, 1, 0, 0, 0).unwrap();
        let t_mid = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(); // between entries
        let t_after = Utc.with_ymd_and_hms(2040, 1, 1, 0, 0, 0).unwrap(); // after all

        assert_eq!(store.still_locked_tokens(t_before), 1000); // retroactively valid
        assert_eq!(store.still_locked_tokens(t_mid), 1000);
        assert_eq!(store.still_locked_tokens(t_after), 100);
    }

    #[test]
    fn test_dense_entry() {
        let json = r#"
            [
                { "timestamp": "2023-10-01T00:00:00Z", "amount_still_locked": 300 },
                { "timestamp": "2023-10-15T00:00:00Z", "amount_still_locked": 200 },
                { "timestamp": "2023-11-01T00:00:00Z", "amount_still_locked": 100 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let t_exact_mid = Utc.with_ymd_and_hms(2023, 10, 15, 0, 0, 0).unwrap();
        let t_between = Utc.with_ymd_and_hms(2023, 10, 20, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(t_exact_mid), 200);
        assert_eq!(store.still_locked_tokens(t_between), 200);
    }

    #[test]
    fn test_first_entry_is_retrospective() {
        let json = r#"
            [
                { "timestamp": "2022-06-01T00:00:00Z", "amount_still_locked": 888 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();

        let far_before = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let just_before = Utc.with_ymd_and_hms(2022, 5, 31, 23, 59, 59).unwrap();
        let exact = Utc.with_ymd_and_hms(2022, 6, 1, 0, 0, 0).unwrap();

        assert_eq!(store.still_locked_tokens(far_before), 888);
        assert_eq!(store.still_locked_tokens(just_before), 888);
        assert_eq!(store.still_locked_tokens(exact), 888);
    }

    #[test]
    fn test_unsorted_input() {
        let json = r#"
            [
                { "timestamp": "2023-11-01T00:00:00Z", "amount_still_locked": 100 },
                { "timestamp": "2023-01-01T00:00:00Z", "amount_still_locked": 300 },
                { "timestamp": "2023-10-01T00:00:00Z", "amount_still_locked": 200 }
            ]
        "#;
        let store = MainnetUnlocksStore::from_json(json).unwrap();
        // The entries should be sorted internally; thus, querying a date between
        // the earliest and the next entry should return the correct value.
        let query_before = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(query_before), 300);
        // Querying exactly at a later entry should return its value.
        let query_exact = Utc.with_ymd_and_hms(2023, 10, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(query_exact), 200);
    }
}
