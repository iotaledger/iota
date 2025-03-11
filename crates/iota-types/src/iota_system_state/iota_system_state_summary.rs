// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::{encoding::Base64, traits::ToFromBytes};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{IotaSystemState, IotaSystemStateTrait};
use crate::{
    base_types::{AuthorityName, IotaAddress, ObjectID},
    committee::{CommitteeWithNetworkMetadata, NetworkMetadata},
    dynamic_field::get_dynamic_field_from_store,
    error::IotaError,
    id::ID,
    iota_serde::{BigInt, Readable},
    iota_system_state::get_validator_from_table,
    multiaddr::Multiaddr,
    storage::ObjectStore,
};

/// This is the JSON-RPC type for IOTA system state objects.
/// It is an enum type that can represent either V1 or V2 system state objects.
#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize, Clone, derive_more::From, JsonSchema)]
pub enum IotaSystemStateSummary {
    V1(IotaSystemStateSummaryV1),
    V2(IotaSystemStateSummaryV2),
}

/// This is the JSON-RPC type for the
/// [`IotaSystemStateV1`](super::iota_system_state_inner_v1::IotaSystemStateV1)
/// object. It flattens all fields to make them top-level fields such that it as
/// minimum dependencies to the internal data structures of the IOTA system
/// state type.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaSystemStateSummaryV1 {
    /// The current epoch ID, starting from 0.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    pub iota_treasury_cap_id: ObjectID,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation rewards accumulated (and not yet distributed)
    /// during safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_rewards: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all active validators at the beginning of the
    /// epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub total_stake: u64,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<IotaValidatorSummary>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    pub pending_active_validators_id: ObjectID,
    /// Number of new validators that will join at the end of the epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[schemars(with = "Vec<BigInt<u64>>")]
    #[serde_as(as = "Vec<Readable<BigInt<u64>, _>>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    pub staking_pool_mappings_id: ObjectID,
    /// Number of staking pool mappings.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    pub inactive_pools_id: ObjectID,
    /// Number of inactive staking pools.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    pub validator_candidates_id: ObjectID,
    /// Number of preactive validators.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[schemars(with = "Vec<(IotaAddress, BigInt<u64>)>")]
    #[serde_as(as = "Vec<(_, Readable<BigInt<u64>, _>)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

/// This is the JSON-RPC type for the
/// [`IotaSystemStateV2`](super::iota_system_state_inner_v2::IotaSystemStateV2)
/// object. It flattens all fields to make them top-level fields such that it as
/// minimum dependencies to the internal data structures of the IOTA system
/// state type.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaSystemStateSummaryV2 {
    /// The current epoch ID, starting from 0.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    pub iota_treasury_cap_id: ObjectID,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation charges accumulated (and not yet distributed)
    /// during safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_charges: u64,
    /// Amount of burned computation charges accumulated during safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_charges_burned: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all active validators at the beginning of the
    /// epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub total_stake: u64,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<IotaValidatorSummary>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    pub pending_active_validators_id: ObjectID,
    /// Number of new validators that will join at the end of the epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[schemars(with = "Vec<BigInt<u64>>")]
    #[serde_as(as = "Vec<Readable<BigInt<u64>, _>>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    pub staking_pool_mappings_id: ObjectID,
    /// Number of staking pool mappings.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    pub inactive_pools_id: ObjectID,
    /// Number of inactive staking pools.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    pub validator_candidates_id: ObjectID,
    /// Number of preactive validators.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[schemars(with = "Vec<(IotaAddress, BigInt<u64>)>")]
    #[serde_as(as = "Vec<(_, Readable<BigInt<u64>, _>)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

impl IotaSystemStateSummary {
    pub fn get_iota_committee_for_benchmarking(&self) -> CommitteeWithNetworkMetadata {
        match self {
            Self::V1(v1) => v1.get_iota_committee_for_benchmarking(),
            Self::V2(v2) => v2.get_iota_committee_for_benchmarking(),
        }
    }
}

impl IotaSystemStateSummaryV1 {
    pub fn get_iota_committee_for_benchmarking(&self) -> CommitteeWithNetworkMetadata {
        let validators = self
            .active_validators
            .iter()
            .map(|validator| {
                let name = AuthorityName::from_bytes(&validator.authority_pubkey_bytes).unwrap();
                (
                    name,
                    (
                        validator.voting_power,
                        NetworkMetadata {
                            network_address: Multiaddr::try_from(validator.net_address.clone())
                                .unwrap(),
                            primary_address: Multiaddr::try_from(validator.primary_address.clone())
                                .unwrap(),
                        },
                    ),
                )
            })
            .collect();
        CommitteeWithNetworkMetadata::new(self.epoch, validators)
    }
}

impl IotaSystemStateSummaryV2 {
    pub fn get_iota_committee_for_benchmarking(&self) -> CommitteeWithNetworkMetadata {
        let validators = self
            .active_validators
            .iter()
            .map(|validator| {
                let name = AuthorityName::from_bytes(&validator.authority_pubkey_bytes).unwrap();
                (
                    name,
                    (
                        validator.voting_power,
                        NetworkMetadata {
                            network_address: Multiaddr::try_from(validator.net_address.clone())
                                .unwrap(),
                            primary_address: Multiaddr::try_from(validator.primary_address.clone())
                                .unwrap(),
                        },
                    ),
                )
            })
            .collect();
        CommitteeWithNetworkMetadata::new(self.epoch, validators)
    }
}

// Conversion traits to make usage and access easier in calling scopes.

impl From<IotaSystemStateSummaryV1> for IotaSystemStateSummaryV2 {
    fn from(v1: IotaSystemStateSummaryV1) -> Self {
        let IotaSystemStateSummaryV1 {
            epoch,
            protocol_version,
            system_state_version,
            iota_total_supply,
            iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance,
            reference_gas_price,
            safe_mode,
            safe_mode_storage_charges,
            safe_mode_computation_rewards,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            min_validator_count,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
            total_stake,
            active_validators,
            pending_active_validators_id,
            pending_active_validators_size,
            pending_removals,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
            at_risk_validators,
            validator_report_records,
        } = v1;
        Self {
            epoch,
            protocol_version,
            system_state_version,
            iota_total_supply,
            iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance,
            reference_gas_price,
            safe_mode,
            safe_mode_storage_charges,
            safe_mode_computation_charges: safe_mode_computation_rewards,
            safe_mode_computation_charges_burned: safe_mode_computation_rewards,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            min_validator_count,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
            total_stake,
            active_validators,
            pending_active_validators_id,
            pending_active_validators_size,
            pending_removals,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
            at_risk_validators,
            validator_report_records,
        }
    }
}

impl From<IotaSystemStateSummaryV2> for IotaSystemStateSummaryV1 {
    fn from(v2: IotaSystemStateSummaryV2) -> Self {
        let IotaSystemStateSummaryV2 {
            epoch,
            protocol_version,
            system_state_version,
            iota_total_supply,
            iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance,
            reference_gas_price,
            safe_mode,
            safe_mode_storage_charges,
            safe_mode_computation_charges,
            safe_mode_computation_charges_burned: _,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            min_validator_count,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
            total_stake,
            active_validators,
            pending_active_validators_id,
            pending_active_validators_size,
            pending_removals,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
            at_risk_validators,
            validator_report_records,
        } = v2;
        Self {
            epoch,
            protocol_version,
            system_state_version,
            iota_total_supply,
            iota_treasury_cap_id,
            storage_fund_total_object_storage_rebates,
            storage_fund_non_refundable_balance,
            reference_gas_price,
            safe_mode,
            safe_mode_storage_charges,
            safe_mode_computation_rewards: safe_mode_computation_charges,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            min_validator_count,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
            total_stake,
            active_validators,
            pending_active_validators_id,
            pending_active_validators_size,
            pending_removals,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
            at_risk_validators,
            validator_report_records,
        }
    }
}

// Conversions from `IotaSystemState` might be fallible in the future.

impl TryFrom<IotaSystemStateSummary> for IotaSystemStateSummaryV1 {
    type Error = IotaError;

    fn try_from(summary: IotaSystemStateSummary) -> Result<Self, Self::Error> {
        Ok(match summary {
            IotaSystemStateSummary::V1(v1) => v1,
            IotaSystemStateSummary::V2(v2) => v2.into(),
        })
    }
}

impl TryFrom<IotaSystemStateSummary> for IotaSystemStateSummaryV2 {
    type Error = IotaError;

    fn try_from(summary: IotaSystemStateSummary) -> Result<Self, Self::Error> {
        Ok(match summary {
            IotaSystemStateSummary::V1(v1) => v1.into(),
            IotaSystemStateSummary::V2(v2) => v2,
        })
    }
}

/// This is the JSON-RPC type for the IOTA validator. It flattens all inner
/// structures to top-level fields so that they are decoupled from the internal
/// definitions.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaValidatorSummary {
    // Metadata
    pub iota_address: IotaAddress,
    #[schemars(with = "Base64")]
    #[serde_as(as = "Base64")]
    pub authority_pubkey_bytes: Vec<u8>,
    #[schemars(with = "Base64")]
    #[serde_as(as = "Base64")]
    pub network_pubkey_bytes: Vec<u8>,
    #[schemars(with = "Base64")]
    #[serde_as(as = "Base64")]
    pub protocol_pubkey_bytes: Vec<u8>,
    #[schemars(with = "Base64")]
    #[serde_as(as = "Base64")]
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    #[schemars(with = "Option<Base64>")]
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_authority_pubkey_bytes: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64>")]
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64>")]
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_network_pubkey_bytes: Option<Vec<u8>>,
    #[schemars(with = "Option<Base64>")]
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_protocol_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_net_address: Option<String>,
    pub next_epoch_p2p_address: Option<String>,
    pub next_epoch_primary_address: Option<String>,

    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub voting_power: u64,
    pub operation_cap_id: ObjectID,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub gas_price: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub commission_rate: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_stake: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_gas_price: u64,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_commission_rate: u64,

    // Staking pool information
    /// ID of the staking pool object.
    pub staking_pool_id: ObjectID,
    /// The epoch at which this pool became active.
    #[schemars(with = "Option<BigInt<u64>>")]
    #[serde_as(as = "Option<Readable<BigInt<u64>, _>>")]
    pub staking_pool_activation_epoch: Option<u64>,
    /// The epoch at which this staking pool ceased to be active. `None` =
    /// {pre-active, active},
    #[schemars(with = "Option<BigInt<u64>>")]
    #[serde_as(as = "Option<Readable<BigInt<u64>, _>>")]
    pub staking_pool_deactivation_epoch: Option<u64>,
    /// The total number of IOTA tokens in this pool.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_iota_balance: u64,
    /// The epoch stake rewards will be added here at the end of each epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub rewards_pool: u64,
    /// Total number of pool tokens issued by the pool.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pool_token_balance: u64,
    /// Pending stake amount for this epoch.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_stake: u64,
    /// Pending stake withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_total_iota_withdraw: u64,
    /// Pending pool token withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_pool_token_withdraw: u64,
    /// ID of the exchange rate table object.
    pub exchange_rates_id: ObjectID,
    /// Number of exchange rates in the table.
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub exchange_rates_size: u64,
}

impl Default for IotaSystemStateSummaryV2 {
    fn default() -> Self {
        IotaSystemStateSummaryV2 {
            epoch: 0,
            protocol_version: 1,
            system_state_version: 1,
            iota_total_supply: 0,
            iota_treasury_cap_id: ObjectID::ZERO,
            storage_fund_total_object_storage_rebates: 0,
            storage_fund_non_refundable_balance: 0,
            reference_gas_price: 1,
            safe_mode: false,
            safe_mode_storage_charges: 0,
            safe_mode_computation_charges: 0,
            safe_mode_computation_charges_burned: 0,
            safe_mode_storage_rebates: 0,
            safe_mode_non_refundable_storage_fee: 0,
            epoch_start_timestamp_ms: 0,
            epoch_duration_ms: 0,
            min_validator_count: 0,
            max_validator_count: 0,
            min_validator_joining_stake: 0,
            validator_low_stake_threshold: 0,
            validator_very_low_stake_threshold: 0,
            validator_low_stake_grace_period: 0,
            total_stake: 0,
            active_validators: vec![],
            pending_active_validators_id: ObjectID::ZERO,
            pending_active_validators_size: 0,
            pending_removals: vec![],
            staking_pool_mappings_id: ObjectID::ZERO,
            staking_pool_mappings_size: 0,
            inactive_pools_id: ObjectID::ZERO,
            inactive_pools_size: 0,
            validator_candidates_id: ObjectID::ZERO,
            validator_candidates_size: 0,
            at_risk_validators: vec![],
            validator_report_records: vec![],
        }
    }
}

impl Default for IotaSystemStateSummary {
    fn default() -> Self {
        Self::V2(Default::default())
    }
}

impl Default for IotaValidatorSummary {
    fn default() -> Self {
        Self {
            iota_address: IotaAddress::default(),
            authority_pubkey_bytes: vec![],
            network_pubkey_bytes: vec![],
            protocol_pubkey_bytes: vec![],
            proof_of_possession_bytes: vec![],
            name: String::new(),
            description: String::new(),
            image_url: String::new(),
            project_url: String::new(),
            net_address: String::new(),
            p2p_address: String::new(),
            primary_address: String::new(),
            next_epoch_authority_pubkey_bytes: None,
            next_epoch_proof_of_possession: None,
            next_epoch_network_pubkey_bytes: None,
            next_epoch_protocol_pubkey_bytes: None,
            next_epoch_net_address: None,
            next_epoch_p2p_address: None,
            next_epoch_primary_address: None,
            voting_power: 0,
            operation_cap_id: ObjectID::ZERO,
            gas_price: 0,
            commission_rate: 0,
            next_epoch_stake: 0,
            next_epoch_gas_price: 0,
            next_epoch_commission_rate: 0,
            staking_pool_id: ObjectID::ZERO,
            staking_pool_activation_epoch: None,
            staking_pool_deactivation_epoch: None,
            staking_pool_iota_balance: 0,
            rewards_pool: 0,
            pool_token_balance: 0,
            pending_stake: 0,
            pending_total_iota_withdraw: 0,
            pending_pool_token_withdraw: 0,
            exchange_rates_id: ObjectID::ZERO,
            exchange_rates_size: 0,
        }
    }
}

/// Given the staking pool id of a validator, return the validator's
/// `IotaValidatorSummary`, works for validator candidates, active validators,
/// as well as inactive validators.
pub fn get_validator_by_pool_id<S>(
    object_store: &S,
    system_state: &IotaSystemState,
    system_state_summary: &IotaSystemStateSummary,
    pool_id: ObjectID,
) -> Result<IotaValidatorSummary, IotaError>
where
    S: ObjectStore + ?Sized,
{
    match system_state_summary {
        IotaSystemStateSummary::V1(summary) => {
            get_validator_by_pool_id_v1(object_store, system_state, summary, pool_id)
        }
        IotaSystemStateSummary::V2(summary) => {
            get_validator_by_pool_id_v2(object_store, system_state, summary, pool_id)
        }
    }
}

fn get_validator_by_pool_id_v1<S>(
    object_store: &S,
    system_state: &IotaSystemState,
    system_state_summary: &IotaSystemStateSummaryV1,
    pool_id: ObjectID,
) -> Result<IotaValidatorSummary, IotaError>
where
    S: ObjectStore + ?Sized,
{
    // First try to find in active validator set.
    let active_validator = system_state_summary
        .active_validators
        .iter()
        .find(|v| v.staking_pool_id == pool_id);
    if let Some(active) = active_validator {
        return Ok(active.clone());
    }
    // Then try to find in pending active validator set.
    let pending_active_validators = system_state.get_pending_active_validators(object_store)?;
    let pending_active = pending_active_validators
        .iter()
        .find(|v| v.staking_pool_id == pool_id);
    if let Some(pending) = pending_active {
        return Ok(pending.clone());
    }
    // After that try to find in inactive pools.
    let inactive_table_id = system_state_summary.inactive_pools_id;
    if let Ok(inactive) =
        get_validator_from_table(&object_store, inactive_table_id, &ID::new(pool_id))
    {
        return Ok(inactive);
    }
    // Finally look up the candidates pool.
    let candidate_address: IotaAddress = get_dynamic_field_from_store(
        &object_store,
        system_state_summary.staking_pool_mappings_id,
        &ID::new(pool_id),
    )
    .map_err(|err| {
        IotaError::IotaSystemStateRead(format!(
            "Failed to load candidate address from pool mappings: {:?}",
            err
        ))
    })?;
    let candidate_table_id = system_state_summary.validator_candidates_id;
    get_validator_from_table(&object_store, candidate_table_id, &candidate_address)
}

fn get_validator_by_pool_id_v2<S>(
    object_store: &S,
    system_state: &IotaSystemState,
    system_state_summary: &IotaSystemStateSummaryV2,
    pool_id: ObjectID,
) -> Result<IotaValidatorSummary, IotaError>
where
    S: ObjectStore + ?Sized,
{
    // First try to find in active validator set.
    let active_validator = system_state_summary
        .active_validators
        .iter()
        .find(|v| v.staking_pool_id == pool_id);
    if let Some(active) = active_validator {
        return Ok(active.clone());
    }
    // Then try to find in pending active validator set.
    let pending_active_validators = system_state.get_pending_active_validators(object_store)?;
    let pending_active = pending_active_validators
        .iter()
        .find(|v| v.staking_pool_id == pool_id);
    if let Some(pending) = pending_active {
        return Ok(pending.clone());
    }
    // After that try to find in inactive pools.
    let inactive_table_id = system_state_summary.inactive_pools_id;
    if let Ok(inactive) =
        get_validator_from_table(&object_store, inactive_table_id, &ID::new(pool_id))
    {
        return Ok(inactive);
    }
    // Finally look up the candidates pool.
    let candidate_address: IotaAddress = get_dynamic_field_from_store(
        &object_store,
        system_state_summary.staking_pool_mappings_id,
        &ID::new(pool_id),
    )
    .map_err(|err| {
        IotaError::IotaSystemStateRead(format!(
            "Failed to load candidate address from pool mappings: {:?}",
            err
        ))
    })?;
    let candidate_table_id = system_state_summary.validator_candidates_id;
    get_validator_from_table(&object_store, candidate_table_id, &candidate_address)
}
