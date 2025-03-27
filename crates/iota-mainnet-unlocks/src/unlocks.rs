// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// File name of the mainnet token unlock data.
const MAINNET_UNLOCK_FILE: &str = "mainnet_unlocks.json";

/// Represents a token unlock entry.
/// Each entry defines how many tokens remain locked at a specific point in
/// time.
#[derive(Debug, Clone, Deserialize)]
pub struct LockedEntry {
    /// UTC timestamp of the interval (e.g. every 2 weeks).
    pub timestamp: DateTime<Utc>,
    /// Total locked amount (nano-units) at this point in time.
    pub amount_locked: u64,
}

/// In-memory store holding pre-aggregated locked supply per 2-week timestamp.
#[derive(Debug, Clone)]
pub struct TokenUnlocksStore {
    /// Map of entries to their unlock schedules.
    entries: BTreeMap<DateTime<Utc>, LockedEntry>,
}

impl TokenUnlocksStore {
    /// Loads all token unlock data from CSV files located in the
    /// `token_unlocks_data` directory at the crate root. Each CSV file
    /// represents the unlock schedule for a single entity.
    ///
    /// Panics if the directory is missing or unreadable.
    pub fn load() -> Self {
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(crate_dir)
            .join("data")
            .join(MAINNET_UNLOCK_FILE);

        let data = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Could not read locked supply file: {:?}", path));

        let parsed: Vec<LockedEntry> =
            serde_json::from_str(&data).unwrap_or_else(|e| panic!("Invalid JSON format: {}", e));

        let mut map = BTreeMap::new();
        for entry in parsed {
            map.insert(entry.timestamp, entry);
        }

        Self { entries: map }
    }

    /// Returns the total amount of tokens (in nano-units) that are still locked
    /// at the given timestamp.
    ///
    /// A token is considered locked if its unlock date is strictly after the
    /// provided time.
    pub fn still_locked_tokens(&self, timestamp: DateTime<Utc>) -> u64 {
        match self.entries.range(timestamp..).next() {
            Some((_, entry)) => entry.amount_locked,
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn mock_store() -> TokenUnlocksStore {
        let mut entries = BTreeMap::new();
        entries.insert(
            Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
            LockedEntry {
                timestamp: Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
                amount_locked: 1500,
            },
        );
        entries.insert(
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            LockedEntry {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
                amount_locked: 500,
            },
        );

        TokenUnlocksStore { entries }
    }

    #[test]
    fn test_locked_supply_mid_range() {
        let store = mock_store();
        let ts = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(ts), 1500);
    }

    #[test]
    fn test_locked_supply_after_all() {
        let store = mock_store();
        let ts = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(ts), 500);
    }

    #[test]
    fn test_locked_supply_before_all() {
        let store = mock_store();
        let ts = Utc.with_ymd_and_hms(2022, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(store.still_locked_tokens(ts), 0);
    }
}
