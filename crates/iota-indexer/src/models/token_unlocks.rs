// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Represents a single token unlock entry for an allocation group.
/// Each entry defines how many tokens become available at a specific point in
/// time.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenUnlock {
    /// Amount of tokens to be unlocked (in nano-units).
    #[serde(rename = "Tokens")]
    pub amount: u64,
    /// UTC timestamp at which the tokens become unlocked and enter circulation.
    #[serde(rename = "Unlock Date")]
    pub unlock_date: DateTime<Utc>,
}

/// Represents an entity (e.g. investor, team allocation, foundation wallet)
/// that has one or more scheduled token unlocks.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Unique name or label identifying the entity.
    pub name: String,

    /// List of token unlock entries associated with this entity.
    /// Each entry defines an amount and the timestamp it becomes unlocked.
    pub unlocks: Vec<TokenUnlock>,
}
