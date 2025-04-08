// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{prelude::*, sql_types::BigInt};
use iota_json_rpc_types::ParticipationMetrics;

#[derive(Clone, Debug, Default, QueryableByName)]
pub struct StoredParticipationMetrics {
    #[diesel(sql_type = BigInt)]
    pub total_addresses: i64,
}

impl From<StoredParticipationMetrics> for ParticipationMetrics {
    fn from(metrics: StoredParticipationMetrics) -> Self {
        Self {
            total_addresses: metrics.total_addresses as u64,
        }
    }
}
