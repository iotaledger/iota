// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::ToFromBytes;
use iota_types::{
    base_types::{AuthorityName, EpochId},
    committee::Committee,
    iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaValidatorSummary,
    messages_checkpoint::CheckpointSequenceNumber,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::{
    MoveFunctionName, Page,
    iota_system_state_summary::IotaValidatorSummary as IotaValidatorSummarySchema,
};

pub type EpochPage = Page<EpochInfo, BigInt<u64>>;
pub type EpochMetricsPage = Page<EpochMetrics, BigInt<u64>>;

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EpochInfo {
    /// Epoch number
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: EpochId,
    /// List of validators included in epoch
    #[schemars(with = "Vec<IotaValidatorSummarySchema>")]
    pub validators: Vec<IotaValidatorSummary>,
    /// Count of tx in epoch
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_total_transactions: u64,
    /// First, last checkpoint sequence numbers
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub first_checkpoint_id: CheckpointSequenceNumber,
    /// The timestamp when the epoch started.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_start_timestamp: u64,
    /// The end of epoch information.
    pub end_of_epoch_info: Option<EndOfEpochInfo>,
    /// The reference gas price for the given epoch.
    pub reference_gas_price: Option<u64>,
    /// Committee validators. Each element is an index
    /// pointing to `validators`.
    #[schemars(with = "Vec<String>")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub committee_members: Vec<u64>,
}

impl EpochInfo {
    pub fn committee(&self) -> Result<Committee, fastcrypto::error::FastCryptoError> {
        let mut voting_rights = BTreeMap::new();
        for &i in &self.committee_members {
            let validator = self
                .validators
                .get(i as usize)
                .expect("validators should include committee members");
            let name = AuthorityName::from_bytes(&validator.authority_pubkey_bytes)?;
            voting_rights.insert(name, validator.voting_power);
        }
        Ok(Committee::new(self.epoch, voting_rights))
    }
}

/// A light-weight version of `EpochInfo` for faster loading
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EpochMetrics {
    /// The current epoch ID.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: EpochId,
    /// The total number of transactions in the epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_total_transactions: u64,
    /// The first checkpoint ID of the epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub first_checkpoint_id: CheckpointSequenceNumber,
    /// The timestamp when the epoch started.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_start_timestamp: u64,
    /// The end of epoch information.
    pub end_of_epoch_info: Option<EndOfEpochInfo>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndOfEpochInfo {
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub last_checkpoint_id: CheckpointSequenceNumber,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_end_timestamp: u64,
    /// existing fields from `SystemEpochInfoEventV1` (without epoch)
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_version: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub reference_gas_price: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_stake: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_charge: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_rebate: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_fund_balance: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_gas_fees: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_stake_rewards_distributed: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub burnt_tokens_amount: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub minted_tokens_amount: u64,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMetrics {
    /// Current TPS - Transaction Blocks per Second.
    pub current_tps: f64,
    /// Peak TPS in the past 30 days
    pub tps_30_days: f64,
    /// Total number of packages published in the network
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_packages: u64,
    /// Total number of addresses seen in the network
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_addresses: u64,
    /// Total number of live objects in the network
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_objects: u64,
    /// Current epoch number
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub current_epoch: u64,
    /// Current checkpoint number
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub current_checkpoint: u64,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveCallMetrics {
    /// The count of calls of each function in the last 3 days.
    #[schemars(with = "Vec<(MoveFunctionName, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub rank_3_days: Vec<(MoveFunctionName, usize)>,
    /// The count of calls of each function in the last 7 days.
    #[schemars(with = "Vec<(MoveFunctionName, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub rank_7_days: Vec<(MoveFunctionName, usize)>,
    /// The count of calls of each function in the last 30 days.
    #[schemars(with = "Vec<(MoveFunctionName, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub rank_30_days: Vec<(MoveFunctionName, usize)>,
}

/// Provides metrics about the addresses.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddressMetrics {
    /// The checkpoint sequence number at which the metrics were computed.
    pub checkpoint: u64,
    /// The epoch to which the checkpoint is assigned.
    pub epoch: u64,
    /// The checkpoint timestamp.
    pub timestamp_ms: u64,
    /// The count of sender and recipient addresses.
    pub cumulative_addresses: u64,
    /// The count of sender addresses.
    pub cumulative_active_addresses: u64,
    /// The count of daily unique sender addresses.
    pub daily_active_addresses: u64,
}

/// Provides metrics about the participation in the network.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParticipationMetrics {
    /// The count of distinct addresses with delegated stake.
    pub total_addresses: u64,
}
