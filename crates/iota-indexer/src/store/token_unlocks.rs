// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use chrono::{DateTime, Utc};
use csv::ReaderBuilder;

use crate::models::token_unlocks::{Entity, TokenUnlock};

/// In-memory store holding all token unlock schedules across entities.
/// Allows querying how many tokens are currently locked.
#[derive(Debug, Clone)]
pub struct TokenUnlocksStore {
    /// All known entities with their respective unlock schedules.
    entities: Vec<Entity>,
}

impl TokenUnlocksStore {
    /// Loads all token unlock data from CSV files located in the
    /// `token_unlocks_data` directory at the crate root. Each CSV file
    /// represents the unlock schedule for a single entity.
    ///
    /// Panics if the directory is missing or unreadable.
    pub fn load() -> Self {
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let dir = PathBuf::from(crate_dir).join("token_unlocks_data");

        let paths = fs::read_dir(&dir)
            .unwrap_or_else(|_| panic!("Token Unlock CSV directory not found: {:?}", dir));

        let entities = paths
            .filter_map(|entry| entry.ok())
            .filter(|e| e.path().extension().unwrap_or_default() == "csv")
            .map(|entry| {
                let path = entry.path();
                let file_name = path.file_stem().unwrap().to_string_lossy().to_string();
                let file = fs::File::open(&path)
                    .unwrap_or_else(|_| panic!("Failed to open file {:?}", path));
                let mut rdr = ReaderBuilder::new()
                    .has_headers(true)
                    .delimiter(b',')
                    .from_reader(file);

                let unlocks = rdr
                    .deserialize()
                    .enumerate()
                    .map(|(line_no, res)| {
                        let raw: (String, String) = res.unwrap_or_else(|e| {
                            panic!("Invalid CSV record at line {}. Error: {}", line_no + 2, e)
                        });

                        let amount = raw.0.parse::<u64>().expect("Invalid number");

                        // Multiply with 1000 to count in NANOS
                        let amount = amount * 1000;

                        // Remove " UTC" suffix because it's not valid in ISO 8601 and causes
                        // chrono to fail parsing
                        let datetime_str = raw.1.replace(" UTC", "");
                        let unlock_date =
                            datetime_str.parse::<DateTime<Utc>>().unwrap_or_else(|_| {
                                panic!("Invalid datetime at line {}: '{}'", line_no + 2, raw.1)
                            });

                        TokenUnlock {
                            amount,
                            unlock_date,
                        }
                    })
                    .collect();

                Entity {
                    name: file_name,
                    unlocks,
                }
            })
            .collect();

        Self { entities }
    }

    /// Returns the total amount of tokens (in nano-units) that are still locked
    /// at the given timestamp.
    ///
    /// A token is considered locked if its unlock date is strictly after the
    /// provided time.
    pub fn still_locked_tokens(&self, timestamp: DateTime<Utc>) -> u64 {
        self.entities
            .iter()
            .flat_map(|inv| &inv.unlocks)
            .filter(|unlock| unlock.unlock_date > timestamp)
            .map(|unlock| unlock.amount)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::models::token_unlocks::TokenUnlock;

    fn mock_store() -> TokenUnlocksStore {
        TokenUnlocksStore {
            entities: vec![
                Entity {
                    name: "Investor A".into(),
                    unlocks: vec![
                        TokenUnlock {
                            amount: 1000,
                            unlock_date: Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
                        },
                        TokenUnlock {
                            amount: 500,
                            unlock_date: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
                        },
                    ],
                },
                Entity {
                    name: "Investor B".into(),
                    unlocks: vec![TokenUnlock {
                        amount: 2000,
                        unlock_date: Utc.with_ymd_and_hms(2022, 6, 1, 0, 0, 0).unwrap(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_locked_supply_before_any_unlock() {
        let store = mock_store();
        let timestamp = Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap();
        let locked = store.still_locked_tokens(timestamp);
        assert_eq!(locked, 3500);
    }

    #[test]
    fn test_locked_supply_after_some_unlocks() {
        let store = mock_store();
        let timestamp = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
        let locked = store.still_locked_tokens(timestamp);
        assert_eq!(locked, 500);
    }

    #[test]
    fn test_locked_supply_after_all_unlocks() {
        let store = mock_store();
        let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let locked = store.still_locked_tokens(timestamp);
        assert_eq!(locked, 0);
    }
}
