// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::{AuthorityName, EpochId, IotaAddress},
    committee::{Committee, StakeUnit},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::{
    IotaAuthorityPublicKeyBytes,
    iota_primitives::{IotaAddress as IotaAddressSchema, ObjectID as ObjectIDSchema},
};

/// RPC representation of the [Committee] type.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename = "CommitteeInfo")]
pub struct IotaCommittee {
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: EpochId,
    #[schemars(with = "Vec<(IotaAuthorityPublicKeyBytes, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub validators: Vec<(AuthorityName, StakeUnit)>,
}

impl From<Committee> for IotaCommittee {
    fn from(committee: Committee) -> Self {
        Self {
            epoch: committee.epoch,
            validators: committee.voting_rights.into_iter().collect(),
        }
    }
}

impl From<IotaCommittee> for Committee {
    fn from(iota_committee: IotaCommittee) -> Self {
        Committee::new(
            iota_committee.epoch,
            iota_committee.validators.into_iter().collect(),
        )
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedStake {
    /// Validator's Address.
    #[schemars(with = "IotaAddressSchema")]
    pub validator_address: IotaAddress,
    /// Staking pool object id.
    #[schemars(with = "ObjectIDSchema")]
    pub staking_pool: ObjectId,
    pub stakes: Vec<Stake>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedTimelockedStake {
    #[schemars(with = "IotaAddressSchema")]
    pub validator_address: IotaAddress,
    #[schemars(with = "ObjectIDSchema")]
    pub staking_pool: ObjectId,
    pub stakes: Vec<TimelockedStake>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(tag = "status")]
pub enum StakeStatus {
    Pending,
    #[serde(rename_all = "camelCase")]
    Active {
        #[serde_as(as = "DisplayFromStr")]
        #[schemars(with = "String")]
        estimated_reward: u64,
    },
    Unstaked,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Stake {
    /// ID of the StakedIota receipt object.
    #[schemars(with = "ObjectIDSchema")]
    pub staked_iota_id: ObjectId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub stake_request_epoch: EpochId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub stake_active_epoch: EpochId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub principal: u64,
    #[serde(flatten)]
    pub status: StakeStatus,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelockedStake {
    #[schemars(with = "ObjectIDSchema")]
    pub timelocked_staked_iota_id: ObjectId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub stake_request_epoch: EpochId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub stake_active_epoch: EpochId,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub principal: u64,
    #[serde(flatten)]
    pub status: StakeStatus,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub expiration_timestamp_ms: u64,
    pub label: Option<String>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ValidatorApys {
    pub apys: Vec<ValidatorApy>,
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub epoch: EpochId,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ValidatorApy {
    #[schemars(with = "IotaAddressSchema")]
    pub address: IotaAddress,
    pub apy: f64,
}
