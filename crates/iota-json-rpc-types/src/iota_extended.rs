// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::ToFromBytes;
use iota_types::{
    base_types::{AuthorityName, EpochId, ObjectID},
    committee::Committee,
    iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaValidatorSummary,
    messages_checkpoint::CheckpointSequenceNumber,
};
use move_core_types::identifier::Identifier;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::Page;

pub type EpochPage = Page<EpochInfo, BigInt<u64>>;

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EpochInfo {
    /// epoch number
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub epoch: EpochId,
    /// list of validators included in epoch
    pub validators: Vec<IotaValidatorSummary>,
    /// count of tx in epoch
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub epoch_total_transactions: u64,
    /// first, last checkpoint sequence numbers
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub first_checkpoint_id: CheckpointSequenceNumber,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub epoch_start_timestamp: u64,
    pub end_of_epoch_info: Option<EndOfEpochInfo>,
    pub reference_gas_price: Option<u64>,
}

impl EpochInfo {
    pub fn committee(&self) -> Result<Committee, fastcrypto::error::FastCryptoError> {
        let mut voting_rights = BTreeMap::new();
        for validator in &self.validators {
            let name = AuthorityName::from_bytes(&validator.protocol_pubkey_bytes)?;
            voting_rights.insert(name, validator.voting_power);
        }
        Ok(Committee::new(self.epoch, voting_rights))
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndOfEpochInfo {
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub last_checkpoint_id: CheckpointSequenceNumber,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub epoch_end_timestamp: u64,
    /// existing fields from `SystemEpochInfoEvent` (without epoch)
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub protocol_version: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub reference_gas_price: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub total_stake: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub storage_charge: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub storage_rebate: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub storage_fund_balance: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub total_gas_fees: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub total_stake_rewards_distributed: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub burnt_tokens_amount: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub minted_tokens_amount: u64,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionName {
    pub package: ObjectID,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub module: Identifier,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub function: Identifier,
}
