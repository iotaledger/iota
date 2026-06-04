// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::IotaAddress,
    iota_system_state::iota_system_state_summary::{
        IotaSystemStateSummary as NativeSystemStateSummary,
        IotaSystemStateSummaryV1 as NativeSystemStateSummaryV1,
        IotaSystemStateSummaryV2 as NativeSystemStateSummaryV2,
        IotaValidatorSummary as NativeValidatorSummary,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

use crate::iota_primitives::{
    Base64 as Base64Schema, IotaAddress as IotaAddressSchema, ObjectID as ObjectIDSchema,
};

/// This is the JSON-RPC type for IOTA system state objects.
/// It is an enum type that can represent either V1 or V2 system state objects.
#[non_exhaustive]
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
pub enum IotaSystemStateSummary {
    V1(IotaSystemStateSummaryV1),
    V2(IotaSystemStateSummaryV2),
}

impl SerializeAs<NativeSystemStateSummary> for IotaSystemStateSummary {
    fn serialize_as<S>(source: &NativeSystemStateSummary, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = IotaSystemStateSummary::from(source.clone());
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeSystemStateSummary> for IotaSystemStateSummary {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeSystemStateSummary, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaSystemStateSummary::deserialize(deserializer)?;
        Ok(NativeSystemStateSummary::from(schema))
    }
}

impl From<IotaSystemStateSummary> for NativeSystemStateSummary {
    fn from(schema: IotaSystemStateSummary) -> Self {
        match schema {
            IotaSystemStateSummary::V1(summary_v1) => {
                NativeSystemStateSummary::V1(summary_v1.into())
            }
            IotaSystemStateSummary::V2(summary_v2) => {
                NativeSystemStateSummary::V2(summary_v2.into())
            }
        }
    }
}

impl From<NativeSystemStateSummary> for IotaSystemStateSummary {
    fn from(summary: NativeSystemStateSummary) -> Self {
        match summary {
            NativeSystemStateSummary::V1(summary_v1) => {
                IotaSystemStateSummary::V1(summary_v1.into())
            }
            NativeSystemStateSummary::V2(summary_v2) => {
                IotaSystemStateSummary::V2(summary_v2.into())
            }
            _ => unimplemented!(
                "a new IotaSystemStateSummary enum variant was added and needs to be handled"
            ),
        }
    }
}

/// This is the JSON-RPC type for the
/// [`IotaSystemStateSummaryV1`](iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummaryV1)
/// object.
#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaSystemStateSummaryV1 {
    /// The current epoch ID, starting from 0.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    #[schemars(with = "ObjectIDSchema")]
    pub iota_treasury_cap_id: ObjectId,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation rewards accumulated (and not yet distributed)
    /// during safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_computation_rewards: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all active validators at the beginning of the
    /// epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_stake: u64,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<IotaValidatorSummary>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    #[schemars(with = "ObjectIDSchema")]
    pub pending_active_validators_id: ObjectId,
    /// Number of new validators that will join at the end of the epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[schemars(with = "Vec<String>")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    #[schemars(with = "ObjectIDSchema")]
    pub staking_pool_mappings_id: ObjectId,
    /// Number of staking pool mappings.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    #[schemars(with = "ObjectIDSchema")]
    pub inactive_pools_id: ObjectId,
    /// Number of inactive staking pools.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    #[schemars(with = "ObjectIDSchema")]
    pub validator_candidates_id: ObjectId,
    /// Number of preactive validators.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[schemars(with = "Vec<(IotaAddressSchema, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    #[schemars(with = "Vec<(IotaAddressSchema, Vec<IotaAddressSchema>)>")]
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

impl SerializeAs<NativeSystemStateSummaryV1> for IotaSystemStateSummaryV1 {
    fn serialize_as<S>(
        source: &NativeSystemStateSummaryV1,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = IotaSystemStateSummaryV1::from(source.clone());
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeSystemStateSummaryV1> for IotaSystemStateSummaryV1 {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeSystemStateSummaryV1, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaSystemStateSummaryV1::deserialize(deserializer)?;
        Ok(NativeSystemStateSummaryV1::from(schema))
    }
}

impl From<IotaSystemStateSummaryV1> for NativeSystemStateSummaryV1 {
    fn from(schema: IotaSystemStateSummaryV1) -> Self {
        Self {
            epoch: schema.epoch,
            protocol_version: schema.protocol_version,
            system_state_version: schema.system_state_version,
            iota_total_supply: schema.iota_total_supply,
            iota_treasury_cap_id: schema.iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates: schema
                .storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance: schema.storage_fund_non_refundable_balance,
            reference_gas_price: schema.reference_gas_price,
            safe_mode: schema.safe_mode,
            safe_mode_storage_charges: schema.safe_mode_storage_charges,
            safe_mode_computation_rewards: schema.safe_mode_computation_rewards,
            safe_mode_storage_rebates: schema.safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee: schema.safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms: schema.epoch_start_timestamp_ms,
            epoch_duration_ms: schema.epoch_duration_ms,
            min_validator_count: schema.min_validator_count,
            max_validator_count: schema.max_validator_count,
            min_validator_joining_stake: schema.min_validator_joining_stake,
            validator_low_stake_threshold: schema.validator_low_stake_threshold,
            validator_very_low_stake_threshold: schema.validator_very_low_stake_threshold,
            validator_low_stake_grace_period: schema.validator_low_stake_grace_period,
            total_stake: schema.total_stake,
            active_validators: schema
                .active_validators
                .into_iter()
                .map(Into::into)
                .collect(),
            pending_active_validators_id: schema.pending_active_validators_id,
            pending_active_validators_size: schema.pending_active_validators_size,
            pending_removals: schema.pending_removals,
            staking_pool_mappings_id: schema.staking_pool_mappings_id,
            staking_pool_mappings_size: schema.staking_pool_mappings_size,
            inactive_pools_id: schema.inactive_pools_id,
            inactive_pools_size: schema.inactive_pools_size,
            validator_candidates_id: schema.validator_candidates_id,
            validator_candidates_size: schema.validator_candidates_size,
            at_risk_validators: schema.at_risk_validators,
            validator_report_records: schema.validator_report_records,
        }
    }
}

impl From<NativeSystemStateSummaryV1> for IotaSystemStateSummaryV1 {
    fn from(summary: NativeSystemStateSummaryV1) -> Self {
        Self {
            epoch: summary.epoch,
            protocol_version: summary.protocol_version,
            system_state_version: summary.system_state_version,
            iota_total_supply: summary.iota_total_supply,
            iota_treasury_cap_id: summary.iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates: summary
                .storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance: summary.storage_fund_non_refundable_balance,
            reference_gas_price: summary.reference_gas_price,
            safe_mode: summary.safe_mode,
            safe_mode_storage_charges: summary.safe_mode_storage_charges,
            safe_mode_computation_rewards: summary.safe_mode_computation_rewards,
            safe_mode_storage_rebates: summary.safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee: summary.safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms: summary.epoch_start_timestamp_ms,
            epoch_duration_ms: summary.epoch_duration_ms,
            min_validator_count: summary.min_validator_count,
            max_validator_count: summary.max_validator_count,
            min_validator_joining_stake: summary.min_validator_joining_stake,
            validator_low_stake_threshold: summary.validator_low_stake_threshold,
            validator_very_low_stake_threshold: summary.validator_very_low_stake_threshold,
            validator_low_stake_grace_period: summary.validator_low_stake_grace_period,
            total_stake: summary.total_stake,
            active_validators: summary
                .active_validators
                .into_iter()
                .map(Into::into)
                .collect(),
            pending_active_validators_id: summary.pending_active_validators_id,
            pending_active_validators_size: summary.pending_active_validators_size,
            pending_removals: summary.pending_removals,
            staking_pool_mappings_id: summary.staking_pool_mappings_id,
            staking_pool_mappings_size: summary.staking_pool_mappings_size,
            inactive_pools_id: summary.inactive_pools_id,
            inactive_pools_size: summary.inactive_pools_size,
            validator_candidates_id: summary.validator_candidates_id,
            validator_candidates_size: summary.validator_candidates_size,
            at_risk_validators: summary.at_risk_validators,
            validator_report_records: summary.validator_report_records,
        }
    }
}

/// This is the JSON-RPC type for the
/// [`IotaSystemStateSummaryV2`](iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummaryV2)
/// object.
#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaSystemStateSummaryV2 {
    /// The current epoch ID, starting from 0.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    #[schemars(with = "ObjectIDSchema")]
    pub iota_treasury_cap_id: ObjectId,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation charges accumulated (and not yet distributed)
    /// during safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_computation_charges: u64,
    /// Amount of burned computation charges accumulated during safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_computation_charges_burned: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all committee validators at the beginning of
    /// the epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub total_stake: u64,
    /// List of committee validators in the current epoch. Each element is an
    /// index pointing to `active_validators`.
    #[schemars(with = "Vec<String>")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub committee_members: Vec<u64>,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<IotaValidatorSummary>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    #[schemars(with = "ObjectIDSchema")]
    pub pending_active_validators_id: ObjectId,
    /// Number of new validators that will join at the end of the epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[schemars(with = "Vec<String>")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    #[schemars(with = "ObjectIDSchema")]
    pub staking_pool_mappings_id: ObjectId,
    /// Number of staking pool mappings.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    #[schemars(with = "ObjectIDSchema")]
    pub inactive_pools_id: ObjectId,
    /// Number of inactive staking pools.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    #[schemars(with = "ObjectIDSchema")]
    pub validator_candidates_id: ObjectId,
    /// Number of preactive validators.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[schemars(with = "Vec<(IotaAddressSchema, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    #[schemars(with = "Vec<(IotaAddressSchema, Vec<IotaAddressSchema>)>")]
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

impl SerializeAs<NativeSystemStateSummaryV2> for IotaSystemStateSummaryV2 {
    fn serialize_as<S>(
        source: &NativeSystemStateSummaryV2,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = IotaSystemStateSummaryV2::from(source.clone());
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeSystemStateSummaryV2> for IotaSystemStateSummaryV2 {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeSystemStateSummaryV2, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaSystemStateSummaryV2::deserialize(deserializer)?;
        Ok(NativeSystemStateSummaryV2::from(schema))
    }
}

impl From<IotaSystemStateSummaryV2> for NativeSystemStateSummaryV2 {
    fn from(schema: IotaSystemStateSummaryV2) -> Self {
        Self {
            epoch: schema.epoch,
            protocol_version: schema.protocol_version,
            system_state_version: schema.system_state_version,
            iota_total_supply: schema.iota_total_supply,
            iota_treasury_cap_id: schema.iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates: schema
                .storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance: schema.storage_fund_non_refundable_balance,
            reference_gas_price: schema.reference_gas_price,
            safe_mode: schema.safe_mode,
            safe_mode_storage_charges: schema.safe_mode_storage_charges,
            safe_mode_computation_charges: schema.safe_mode_computation_charges,
            safe_mode_computation_charges_burned: schema.safe_mode_computation_charges_burned,
            safe_mode_storage_rebates: schema.safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee: schema.safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms: schema.epoch_start_timestamp_ms,
            epoch_duration_ms: schema.epoch_duration_ms,
            min_validator_count: schema.min_validator_count,
            max_validator_count: schema.max_validator_count,
            min_validator_joining_stake: schema.min_validator_joining_stake,
            validator_low_stake_threshold: schema.validator_low_stake_threshold,
            validator_very_low_stake_threshold: schema.validator_very_low_stake_threshold,
            validator_low_stake_grace_period: schema.validator_low_stake_grace_period,
            total_stake: schema.total_stake,
            committee_members: schema.committee_members,
            active_validators: schema
                .active_validators
                .into_iter()
                .map(Into::into)
                .collect(),
            pending_active_validators_id: schema.pending_active_validators_id,
            pending_active_validators_size: schema.pending_active_validators_size,
            pending_removals: schema.pending_removals,
            staking_pool_mappings_id: schema.staking_pool_mappings_id,
            staking_pool_mappings_size: schema.staking_pool_mappings_size,
            inactive_pools_id: schema.inactive_pools_id,
            inactive_pools_size: schema.inactive_pools_size,
            validator_candidates_id: schema.validator_candidates_id,
            validator_candidates_size: schema.validator_candidates_size,
            at_risk_validators: schema.at_risk_validators,
            validator_report_records: schema.validator_report_records,
        }
    }
}

impl From<NativeSystemStateSummaryV2> for IotaSystemStateSummaryV2 {
    fn from(summary: NativeSystemStateSummaryV2) -> Self {
        Self {
            epoch: summary.epoch,
            protocol_version: summary.protocol_version,
            system_state_version: summary.system_state_version,
            iota_total_supply: summary.iota_total_supply,
            iota_treasury_cap_id: summary.iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates: summary
                .storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance: summary.storage_fund_non_refundable_balance,
            reference_gas_price: summary.reference_gas_price,
            safe_mode: summary.safe_mode,
            safe_mode_storage_charges: summary.safe_mode_storage_charges,
            safe_mode_computation_charges: summary.safe_mode_computation_charges,
            safe_mode_computation_charges_burned: summary.safe_mode_computation_charges_burned,
            safe_mode_storage_rebates: summary.safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee: summary.safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms: summary.epoch_start_timestamp_ms,
            epoch_duration_ms: summary.epoch_duration_ms,
            min_validator_count: summary.min_validator_count,
            max_validator_count: summary.max_validator_count,
            min_validator_joining_stake: summary.min_validator_joining_stake,
            validator_low_stake_threshold: summary.validator_low_stake_threshold,
            validator_very_low_stake_threshold: summary.validator_very_low_stake_threshold,
            validator_low_stake_grace_period: summary.validator_low_stake_grace_period,
            total_stake: summary.total_stake,
            committee_members: summary.committee_members,
            active_validators: summary
                .active_validators
                .into_iter()
                .map(Into::into)
                .collect(),
            pending_active_validators_id: summary.pending_active_validators_id,
            pending_active_validators_size: summary.pending_active_validators_size,
            pending_removals: summary.pending_removals,
            staking_pool_mappings_id: summary.staking_pool_mappings_id,
            staking_pool_mappings_size: summary.staking_pool_mappings_size,
            inactive_pools_id: summary.inactive_pools_id,
            inactive_pools_size: summary.inactive_pools_size,
            validator_candidates_id: summary.validator_candidates_id,
            validator_candidates_size: summary.validator_candidates_size,
            at_risk_validators: summary.at_risk_validators,
            validator_report_records: summary.validator_report_records,
        }
    }
}

/// This is the JSON-RPC type for the IOTA validator. It flattens all inner
/// structures to top-level fields so that they are decoupled from the internal
/// definitions.
#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaValidatorSummary {
    // Metadata
    #[schemars(with = "IotaAddressSchema")]
    pub iota_address: IotaAddress,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub authority_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub network_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub protocol_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    #[schemars(with = "Option<Base64Schema>")]
    pub next_epoch_authority_pubkey_bytes: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64Schema>")]
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64Schema>")]
    pub next_epoch_network_pubkey_bytes: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64Schema>")]
    pub next_epoch_protocol_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_net_address: Option<String>,
    pub next_epoch_p2p_address: Option<String>,
    pub next_epoch_primary_address: Option<String>,

    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub voting_power: u64,
    #[schemars(with = "ObjectIDSchema")]
    pub operation_cap_id: ObjectId,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub gas_price: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub commission_rate: u64,
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub effective_commission_rate: Option<u64>,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub next_epoch_stake: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub next_epoch_gas_price: u64,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub next_epoch_commission_rate: u64,

    // Staking pool information
    /// ID of the staking pool object.
    #[schemars(with = "ObjectIDSchema")]
    pub staking_pool_id: ObjectId,
    /// The epoch at which this pool became active.
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub staking_pool_activation_epoch: Option<u64>,
    /// The epoch at which this staking pool ceased to be active. `None` =
    /// {pre-active, active},
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub staking_pool_deactivation_epoch: Option<u64>,
    /// The total number of IOTA tokens in this pool.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub staking_pool_iota_balance: u64,
    /// The epoch stake rewards will be added here at the end of each epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub rewards_pool: u64,
    /// Total number of pool tokens issued by the pool.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pool_token_balance: u64,
    /// Pending stake amount for this epoch.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pending_stake: u64,
    /// Pending stake withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pending_total_iota_withdraw: u64,
    /// Pending pool token withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub pending_pool_token_withdraw: u64,
    /// ID of the exchange rate table object.
    #[schemars(with = "ObjectIDSchema")]
    pub exchange_rates_id: ObjectId,
    /// Number of exchange rates in the table.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub exchange_rates_size: u64,
}

impl SerializeAs<NativeValidatorSummary> for IotaValidatorSummary {
    fn serialize_as<S>(source: &NativeValidatorSummary, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = IotaValidatorSummary::from(source.clone());
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeValidatorSummary> for IotaValidatorSummary {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeValidatorSummary, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaValidatorSummary::deserialize(deserializer)?;
        Ok(NativeValidatorSummary::from(schema))
    }
}

impl From<NativeValidatorSummary> for IotaValidatorSummary {
    fn from(summary: NativeValidatorSummary) -> Self {
        Self {
            iota_address: summary.iota_address,
            authority_pubkey_bytes: summary.authority_pubkey_bytes,
            network_pubkey_bytes: summary.network_pubkey_bytes,
            protocol_pubkey_bytes: summary.protocol_pubkey_bytes,
            proof_of_possession_bytes: summary.proof_of_possession_bytes,
            name: summary.name,
            description: summary.description,
            image_url: summary.image_url,
            project_url: summary.project_url,
            net_address: summary.net_address,
            p2p_address: summary.p2p_address,
            primary_address: summary.primary_address,
            next_epoch_authority_pubkey_bytes: summary.next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession: summary.next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes: summary.next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes: summary.next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address: summary.next_epoch_net_address,
            next_epoch_p2p_address: summary.next_epoch_p2p_address,
            next_epoch_primary_address: summary.next_epoch_primary_address,
            voting_power: summary.voting_power,
            operation_cap_id: summary.operation_cap_id,
            gas_price: summary.gas_price,
            commission_rate: summary.commission_rate,
            effective_commission_rate: summary.effective_commission_rate,
            next_epoch_stake: summary.next_epoch_stake,
            next_epoch_gas_price: summary.next_epoch_gas_price,
            next_epoch_commission_rate: summary.next_epoch_commission_rate,
            staking_pool_id: summary.staking_pool_id,
            staking_pool_activation_epoch: summary.staking_pool_activation_epoch,
            staking_pool_deactivation_epoch: summary.staking_pool_deactivation_epoch,
            staking_pool_iota_balance: summary.staking_pool_iota_balance,
            rewards_pool: summary.rewards_pool,
            pool_token_balance: summary.pool_token_balance,
            pending_stake: summary.pending_stake,
            pending_total_iota_withdraw: summary.pending_total_iota_withdraw,
            pending_pool_token_withdraw: summary.pending_pool_token_withdraw,
            exchange_rates_id: summary.exchange_rates_id,
            exchange_rates_size: summary.exchange_rates_size,
        }
    }
}

impl From<IotaValidatorSummary> for NativeValidatorSummary {
    fn from(schema: IotaValidatorSummary) -> Self {
        Self {
            iota_address: schema.iota_address,
            authority_pubkey_bytes: schema.authority_pubkey_bytes,
            network_pubkey_bytes: schema.network_pubkey_bytes,
            protocol_pubkey_bytes: schema.protocol_pubkey_bytes,
            proof_of_possession_bytes: schema.proof_of_possession_bytes,
            name: schema.name,
            description: schema.description,
            image_url: schema.image_url,
            project_url: schema.project_url,
            net_address: schema.net_address,
            p2p_address: schema.p2p_address,
            primary_address: schema.primary_address,
            next_epoch_authority_pubkey_bytes: schema.next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession: schema.next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes: schema.next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes: schema.next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address: schema.next_epoch_net_address,
            next_epoch_p2p_address: schema.next_epoch_p2p_address,
            next_epoch_primary_address: schema.next_epoch_primary_address,
            voting_power: schema.voting_power,
            operation_cap_id: schema.operation_cap_id,
            gas_price: schema.gas_price,
            commission_rate: schema.commission_rate,
            effective_commission_rate: schema.effective_commission_rate,
            next_epoch_stake: schema.next_epoch_stake,
            next_epoch_gas_price: schema.next_epoch_gas_price,
            next_epoch_commission_rate: schema.next_epoch_commission_rate,
            staking_pool_id: schema.staking_pool_id,
            staking_pool_activation_epoch: schema.staking_pool_activation_epoch,
            staking_pool_deactivation_epoch: schema.staking_pool_deactivation_epoch,
            staking_pool_iota_balance: schema.staking_pool_iota_balance,
            rewards_pool: schema.rewards_pool,
            pool_token_balance: schema.pool_token_balance,
            pending_stake: schema.pending_stake,
            pending_total_iota_withdraw: schema.pending_total_iota_withdraw,
            pending_pool_token_withdraw: schema.pending_pool_token_withdraw,
            exchange_rates_id: schema.exchange_rates_id,
            exchange_rates_size: schema.exchange_rates_size,
        }
    }
}
