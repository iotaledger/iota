// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{
    Insertable, Queryable, Selectable,
    prelude::{AsChangeset, Identifiable},
};
use iota_json_rpc_types::{EndOfEpochInfo, EpochInfo};
use iota_types::{
    iota_system_state::iota_system_state_summary::{
        IotaSystemStateSummary, IotaSystemStateSummaryV1,
    },
    messages_checkpoint::CertifiedCheckpointSummary,
};

use crate::{
    errors::IndexerError,
    schema::{epochs, feature_flags, protocol_configs},
    types::IndexedEpochInfoEvent,
};

#[derive(Queryable, Insertable, Debug, Clone, Default)]
#[diesel(table_name = epochs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct StoredEpochInfo {
    pub epoch: i64,
    pub first_checkpoint_id: i64,
    pub epoch_start_timestamp: i64,
    pub reference_gas_price: i64,
    pub protocol_version: i64,
    pub total_stake: i64,
    pub storage_fund_balance: i64,
    pub system_state: Vec<u8>,
    /// Total number of network transactions at the end of the epoch.
    pub network_total_transactions: Option<i64>,
    pub last_checkpoint_id: Option<i64>,
    pub epoch_end_timestamp: Option<i64>,
    pub storage_charge: Option<i64>,
    pub storage_rebate: Option<i64>,
    pub total_gas_fees: Option<i64>,
    pub total_stake_rewards_distributed: Option<i64>,
    pub epoch_commitments: Option<Vec<u8>>,
    pub burnt_tokens_amount: Option<i64>,
    pub minted_tokens_amount: Option<i64>,
    /// First transaction sequence number of this epoch.
    pub first_tx_sequence_number: i64,
}

impl StoredEpochInfo {
    pub fn epoch_total_transactions(&self) -> Option<i64> {
        self.network_total_transactions
            .map(|total_tx| total_tx - self.first_tx_sequence_number)
    }
}

#[derive(Queryable, Insertable, Debug, Clone, Default)]
#[diesel(table_name = protocol_configs)]
pub struct StoredProtocolConfig {
    pub protocol_version: i64,
    pub config_name: String,
    pub config_value: Option<String>,
}

#[derive(Queryable, Insertable, Debug, Clone, Default)]
#[diesel(table_name = feature_flags)]
pub struct StoredFeatureFlag {
    pub protocol_version: i64,
    pub flag_name: String,
    pub flag_value: bool,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = epochs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct QueryableEpochInfo {
    pub epoch: i64,
    pub first_checkpoint_id: i64,
    pub epoch_start_timestamp: i64,
    pub reference_gas_price: i64,
    pub protocol_version: i64,
    pub total_stake: i64,
    pub storage_fund_balance: i64,
    pub network_total_transactions: Option<i64>,
    pub last_checkpoint_id: Option<i64>,
    pub epoch_end_timestamp: Option<i64>,
    pub storage_charge: Option<i64>,
    pub storage_rebate: Option<i64>,
    pub total_gas_fees: Option<i64>,
    pub total_stake_rewards_distributed: Option<i64>,
    pub epoch_commitments: Option<Vec<u8>>,
    pub burnt_tokens_amount: Option<i64>,
    pub minted_tokens_amount: Option<i64>,
    pub first_tx_sequence_number: i64,
}

impl QueryableEpochInfo {
    pub fn epoch_total_transactions(&self) -> Option<i64> {
        self.network_total_transactions
            .map(|total_tx| total_tx - self.first_tx_sequence_number)
    }
}

#[derive(Queryable)]
pub struct QueryableEpochSystemState {
    pub epoch: i64,
    pub system_state: Vec<u8>,
}

#[derive(Insertable, Identifiable, AsChangeset, Clone, Debug)]
#[diesel(primary_key(epoch))]
#[diesel(table_name = epochs)]
pub(crate) struct StartOfEpochUpdate {
    pub epoch: i64,
    pub first_checkpoint_id: i64,
    pub first_tx_sequence_number: i64,
    pub epoch_start_timestamp: i64,
    pub reference_gas_price: i64,
    pub protocol_version: i64,
    pub total_stake: i64,
    pub storage_fund_balance: i64,
    pub system_state: Vec<u8>,
}

#[derive(Identifiable, AsChangeset, Clone, Debug)]
#[diesel(primary_key(epoch))]
#[diesel(table_name = epochs)]
pub(crate) struct EndOfEpochUpdate {
    pub epoch: i64,
    pub network_total_transactions: i64,
    pub last_checkpoint_id: i64,
    pub epoch_end_timestamp: i64,
    pub storage_charge: i64,
    pub storage_rebate: i64,
    pub total_gas_fees: i64,
    pub total_stake_rewards_distributed: i64,
    pub epoch_commitments: Vec<u8>,
    pub burnt_tokens_amount: i64,
    pub minted_tokens_amount: i64,
}

impl StartOfEpochUpdate {
    pub fn new(
        new_system_state_summary: &IotaSystemStateSummary,
        first_checkpoint_id: u64,
        first_tx_sequence_number: u64,
        event: Option<&IndexedEpochInfoEvent>,
    ) -> Self {
        // NOTE: total_stake and storage_fund_balance are about new epoch,
        // although the event is generated at the end of the previous epoch,
        // the event is optional b/c no such event for the first epoch.
        let (total_stake, storage_fund_balance) = match event {
            Some(event) => (event.total_stake, event.storage_fund_balance),
            None => (0, 0),
        };
        Self {
            epoch: new_system_state_summary.epoch() as i64,
            first_checkpoint_id: first_checkpoint_id as i64,
            first_tx_sequence_number: first_tx_sequence_number as i64,
            epoch_start_timestamp: new_system_state_summary.epoch_start_timestamp_ms() as i64,
            reference_gas_price: new_system_state_summary.reference_gas_price() as i64,
            protocol_version: new_system_state_summary.protocol_version().as_u64() as i64,
            total_stake: total_stake as i64,
            storage_fund_balance: storage_fund_balance as i64,
            system_state: bcs::to_bytes(new_system_state_summary).unwrap(),
        }
    }
}

impl EndOfEpochUpdate {
    pub fn new(
        last_checkpoint_summary: &CertifiedCheckpointSummary,
        event: &IndexedEpochInfoEvent,
    ) -> Self {
        Self {
            epoch: last_checkpoint_summary.epoch as i64,
            network_total_transactions: last_checkpoint_summary.network_total_transactions as i64,
            last_checkpoint_id: *last_checkpoint_summary.sequence_number() as i64,
            epoch_end_timestamp: last_checkpoint_summary.timestamp_ms as i64,
            storage_charge: event.storage_charge as i64,
            storage_rebate: event.storage_rebate as i64,
            total_gas_fees: event.total_gas_fees as i64,
            total_stake_rewards_distributed: event.total_stake_rewards_distributed as i64,
            epoch_commitments: bcs::to_bytes(
                &last_checkpoint_summary
                    .end_of_epoch_data
                    .clone()
                    .unwrap()
                    .epoch_commitments,
            )
            .unwrap(),
            burnt_tokens_amount: event.burnt_tokens_amount as i64,
            minted_tokens_amount: event.minted_tokens_amount as i64,
        }
    }
}

impl From<&StoredEpochInfo> for Option<EndOfEpochInfo> {
    fn from(info: &StoredEpochInfo) -> Option<EndOfEpochInfo> {
        Some(EndOfEpochInfo {
            reference_gas_price: (info.reference_gas_price as u64),
            protocol_version: (info.protocol_version as u64),
            last_checkpoint_id: info.last_checkpoint_id.map(|v| v as u64)?,
            total_stake: info.total_stake as u64,
            storage_fund_balance: info.storage_fund_balance as u64,
            epoch_end_timestamp: info.epoch_end_timestamp.map(|v| v as u64)?,
            storage_charge: info.storage_charge.map(|v| v as u64)?,
            storage_rebate: info.storage_rebate.map(|v| v as u64)?,
            total_gas_fees: info.total_gas_fees.map(|v| v as u64)?,
            total_stake_rewards_distributed: info
                .total_stake_rewards_distributed
                .map(|v| v as u64)?,
            burnt_tokens_amount: info.burnt_tokens_amount.map(|v| v as u64)?,
            minted_tokens_amount: info.minted_tokens_amount.map(|v| v as u64)?,
        })
    }
}

impl TryFrom<&StoredEpochInfo> for IotaSystemStateSummary {
    type Error = IndexerError;

    fn try_from(value: &StoredEpochInfo) -> Result<Self, Self::Error> {
        IotaSystemStateSummaryV1::try_from(value)
            .map(Into::into)
            .or_else(|_| {
                bcs::from_bytes(&value.system_state).map_err(|_| {
                    IndexerError::PersistentStorageDataCorruption(
                        "failed to deserialize `system_state`".into(),
                    )
                })
            })
    }
}

impl TryFrom<&StoredEpochInfo> for IotaSystemStateSummaryV1 {
    type Error = IndexerError;

    fn try_from(value: &StoredEpochInfo) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.system_state).map_err(|_| {
            IndexerError::PersistentStorageDataCorruption(
                "failed to deserialize `system_state`".into(),
            )
        })
    }
}

impl TryFrom<StoredEpochInfo> for EpochInfo {
    type Error = IndexerError;

    fn try_from(value: StoredEpochInfo) -> Result<Self, Self::Error> {
        let epoch = value.epoch as u64;
        let end_of_epoch_info = (&value).into();
        let system_state = IotaSystemStateSummary::try_from(&value).map_err(|_| {
            IndexerError::PersistentStorageDataCorruption(format!(
                "failed to deserialize `system_state` for epoch {epoch}",
            ))
        })?;
        Ok(EpochInfo {
            epoch: value.epoch as u64,
            validators: system_state.active_validators().to_vec(),
            epoch_total_transactions: value.epoch_total_transactions().unwrap_or(0) as u64,
            first_checkpoint_id: value.first_checkpoint_id as u64,
            epoch_start_timestamp: value.epoch_start_timestamp as u64,
            end_of_epoch_info,
            reference_gas_price: Some(value.reference_gas_price as u64),
            committee_members: system_state.committee_members(),
        })
    }
}
