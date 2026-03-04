// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_protocol_config::PROTOCOL_VERSION_IIP8;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    iota_serde::{BigInt, Readable},
    iota_system_state::iota_system_state_summary::{
        IotaSystemStateSummary, IotaSystemStateSummaryV1, IotaSystemStateSummaryV2,
        IotaValidatorSummary,
    },
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// The representation of system state.
#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize, Clone, derive_more::From)]
pub enum StoredSystemState {
    V1(StoredSystemStateV1),
    V2(StoredSystemStateV2),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredSystemStateV1 {
    /// The current epoch ID, starting from 0.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    pub iota_treasury_cap_id: ObjectID,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation rewards accumulated (and not yet distributed)
    /// during safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_rewards: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all active validators at the beginning of the
    /// epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub total_stake: u64,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<StoredValidator>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    pub pending_active_validators_id: ObjectID,
    /// Number of new validators that will join at the end of the epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[serde_as(as = "Vec<Readable<BigInt<u64>, _>>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    pub staking_pool_mappings_id: ObjectID,
    /// Number of staking pool mappings.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    pub inactive_pools_id: ObjectID,
    /// Number of inactive staking pools.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    pub validator_candidates_id: ObjectID,
    /// Number of preactive validators.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[serde_as(as = "Vec<(_, Readable<BigInt<u64>, _>)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredSystemStateV2 {
    /// The current epoch ID, starting from 0.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch: u64,
    /// The current protocol version, starting from 1.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub protocol_version: u64,
    /// The current version of the system state data structure type.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub system_state_version: u64,
    /// The current IOTA supply.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub iota_total_supply: u64,
    /// The `TreasuryCap<IOTA>` object ID.
    pub iota_treasury_cap_id: ObjectID,
    /// The storage rebates of all the objects on-chain stored in the storage
    /// fund.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_total_object_storage_rebates: u64,
    /// The non-refundable portion of the storage fund coming from
    /// non-refundable storage rebates and any leftover
    /// staking rewards.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub storage_fund_non_refundable_balance: u64,
    /// The reference gas price for the current epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub reference_gas_price: u64,
    /// Whether the system is running in a downgraded safe mode due to a
    /// non-recoverable bug. This is set whenever we failed to execute
    /// advance_epoch, and ended up executing advance_epoch_safe_mode.
    /// It can be reset once we are able to successfully execute advance_epoch.
    pub safe_mode: bool,
    /// Amount of storage charges accumulated (and not yet distributed) during
    /// safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_charges: u64,
    /// Amount of computation charges accumulated (and not yet distributed)
    /// during safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_charges: u64,
    /// Amount of burned computation charges accumulated during safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_computation_charges_burned: u64,
    /// Amount of storage rebates accumulated (and not yet burned) during safe
    /// mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_storage_rebates: u64,
    /// Amount of non-refundable storage fee accumulated during safe mode.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub safe_mode_non_refundable_storage_fee: u64,
    /// Unix timestamp of the current epoch start
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_start_timestamp_ms: u64,

    // System parameters
    /// The duration of an epoch, in milliseconds.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go under this.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_low_stake_grace_period: u64,

    // Validator set
    /// Total amount of stake from all committee validators at the beginning of
    /// the epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub total_stake: u64,
    /// List of committee validators in the current epoch. Each element is an
    /// index pointing to `active_validators`.
    #[serde_as(as = "Vec<Readable<BigInt<u64>, _>>")]
    pub committee_members: Vec<u64>,
    /// The list of active validators in the current epoch.
    pub active_validators: Vec<StoredValidator>,
    /// ID of the object that contains the list of new validators that will join
    /// at the end of the epoch.
    pub pending_active_validators_id: ObjectID,
    /// Number of new validators that will join at the end of the epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_active_validators_size: u64,
    /// Removal requests from the validators. Each element is an index
    /// pointing to `active_validators`.
    #[serde_as(as = "Vec<Readable<BigInt<u64>, _>>")]
    pub pending_removals: Vec<u64>,
    /// ID of the object that maps from staking pool's ID to the iota address of
    /// a validator.
    pub staking_pool_mappings_id: ObjectID,
    /// Number of staking pool mappings.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_mappings_size: u64,
    /// ID of the object that maps from a staking pool ID to the inactive
    /// validator that has that pool as its staking pool.
    pub inactive_pools_id: ObjectID,
    /// Number of inactive staking pools.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub inactive_pools_size: u64,
    /// ID of the object that stores preactive validators, mapping their
    /// addresses to their `Validator` structs.
    pub validator_candidates_id: ObjectID,
    /// Number of preactive validators.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub validator_candidates_size: u64,
    /// Map storing the number of epochs for which each validator has been below
    /// the low stake threshold.
    #[serde_as(as = "Vec<(_, Readable<BigInt<u64>, _>)>")]
    pub at_risk_validators: Vec<(IotaAddress, u64)>,
    /// A map storing the records of validator reporting each other.
    pub validator_report_records: Vec<(IotaAddress, Vec<IotaAddress>)>,
}

/// Represent the stored data for IOTA validators.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredValidator {
    // Metadata
    pub iota_address: IotaAddress,
    #[serde_as(as = "Base64")]
    pub authority_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    pub network_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    pub protocol_pubkey_bytes: Vec<u8>,
    #[serde_as(as = "Base64")]
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_authority_pubkey_bytes: Option<Vec<u8>>,
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_network_pubkey_bytes: Option<Vec<u8>>,
    #[serde_as(as = "Option<Base64>")]
    pub next_epoch_protocol_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_net_address: Option<String>,
    pub next_epoch_p2p_address: Option<String>,
    pub next_epoch_primary_address: Option<String>,

    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub voting_power: u64,
    pub operation_cap_id: ObjectID,
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub gas_price: u64,
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub commission_rate: u64,
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_stake: u64,
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_gas_price: u64,
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub next_epoch_commission_rate: u64,

    // Staking pool information
    /// ID of the staking pool object.
    pub staking_pool_id: ObjectID,
    /// The epoch at which this pool became active.
    #[serde_as(as = "Option<Readable<BigInt<u64>, _>>")]
    pub staking_pool_activation_epoch: Option<u64>,
    /// The epoch at which this staking pool ceased to be active. `None` =
    /// {pre-active, active},
    #[serde_as(as = "Option<Readable<BigInt<u64>, _>>")]
    pub staking_pool_deactivation_epoch: Option<u64>,
    /// The total number of IOTA tokens in this pool.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub staking_pool_iota_balance: u64,
    /// The epoch stake rewards will be added here at the end of each epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub rewards_pool: u64,
    /// Total number of pool tokens issued by the pool.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pool_token_balance: u64,
    /// Pending stake amount for this epoch.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_stake: u64,
    /// Pending stake withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_total_iota_withdraw: u64,
    /// Pending pool token withdrawn during the current epoch, emptied at epoch
    /// boundaries.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub pending_pool_token_withdraw: u64,
    /// ID of the exchange rate table object.
    pub exchange_rates_id: ObjectID,
    /// Number of exchange rates in the table.
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub exchange_rates_size: u64,
}

impl From<IotaValidatorSummary> for StoredValidator {
    fn from(native: IotaValidatorSummary) -> Self {
        let IotaValidatorSummary {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession_bytes,
            name,
            description,
            image_url,
            project_url,
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
            voting_power,
            operation_cap_id,
            gas_price,
            commission_rate,
            effective_commission_rate: _,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
            staking_pool_id,
            staking_pool_activation_epoch,
            staking_pool_deactivation_epoch,
            staking_pool_iota_balance,
            rewards_pool,
            pool_token_balance,
            pending_stake,
            pending_total_iota_withdraw,
            pending_pool_token_withdraw,
            exchange_rates_id,
            exchange_rates_size,
        } = native;
        Self {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession_bytes,
            name,
            description,
            image_url,
            project_url,
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
            voting_power,
            operation_cap_id,
            gas_price,
            commission_rate,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
            staking_pool_id,
            staking_pool_activation_epoch,
            staking_pool_deactivation_epoch,
            staking_pool_iota_balance,
            rewards_pool,
            pool_token_balance,
            pending_stake,
            pending_total_iota_withdraw,
            pending_pool_token_withdraw,
            exchange_rates_id,
            exchange_rates_size,
        }
    }
}

impl StoredValidator {
    pub fn into_iota_validator_summary(self, protocol_version: u64) -> IotaValidatorSummary {
        let StoredValidator {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession_bytes,
            name,
            description,
            image_url,
            project_url,
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
            voting_power,
            operation_cap_id,
            gas_price,
            commission_rate,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
            staking_pool_id,
            staking_pool_activation_epoch,
            staking_pool_deactivation_epoch,
            staking_pool_iota_balance,
            rewards_pool,
            pool_token_balance,
            pending_stake,
            pending_total_iota_withdraw,
            pending_pool_token_withdraw,
            exchange_rates_id,
            exchange_rates_size,
        } = self;
        let effective_commission_rate = (protocol_version >= PROTOCOL_VERSION_IIP8)
            .then(|| commission_rate.max(voting_power))
            .or(Some(commission_rate));
        IotaValidatorSummary {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession_bytes,
            name,
            description,
            image_url,
            project_url,
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey_bytes,
            next_epoch_proof_of_possession,
            next_epoch_network_pubkey_bytes,
            next_epoch_protocol_pubkey_bytes,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
            voting_power,
            operation_cap_id,
            gas_price,
            commission_rate,
            effective_commission_rate,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
            staking_pool_id,
            staking_pool_activation_epoch,
            staking_pool_deactivation_epoch,
            staking_pool_iota_balance,
            rewards_pool,
            pool_token_balance,
            pending_stake,
            pending_total_iota_withdraw,
            pending_pool_token_withdraw,
            exchange_rates_id,
            exchange_rates_size,
        }
    }
}

impl From<StoredSystemStateV1> for IotaSystemStateSummaryV1 {
    fn from(stored: StoredSystemStateV1) -> Self {
        let StoredSystemStateV1 {
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
        } = stored;
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
            active_validators: active_validators
                .into_iter()
                .map(|validator| validator.into_iota_validator_summary(protocol_version))
                .collect(),
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

impl From<IotaSystemStateSummaryV1> for StoredSystemStateV1 {
    fn from(native: IotaSystemStateSummaryV1) -> Self {
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
        } = native;
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
            active_validators: active_validators.into_iter().map(Into::into).collect(),
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

impl From<StoredSystemStateV2> for IotaSystemStateSummaryV2 {
    fn from(stored: StoredSystemStateV2) -> Self {
        let StoredSystemStateV2 {
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
            safe_mode_computation_charges_burned,
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
            committee_members,
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
        } = stored;
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
            safe_mode_computation_charges,
            safe_mode_computation_charges_burned,
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
            committee_members,
            active_validators: active_validators
                .into_iter()
                .map(|validator| validator.into_iota_validator_summary(protocol_version))
                .collect(),
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

impl From<IotaSystemStateSummaryV2> for StoredSystemStateV2 {
    fn from(native: IotaSystemStateSummaryV2) -> Self {
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
            safe_mode_computation_charges_burned,
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
            committee_members,
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
        } = native;
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
            safe_mode_computation_charges,
            safe_mode_computation_charges_burned,
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
            committee_members,
            active_validators: active_validators.into_iter().map(Into::into).collect(),
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

impl From<StoredSystemState> for IotaSystemStateSummary {
    fn from(stored: StoredSystemState) -> Self {
        match stored {
            StoredSystemState::V1(inner) => Self::V1(inner.into()),
            StoredSystemState::V2(inner) => Self::V2(inner.into()),
        }
    }
}

impl From<IotaSystemStateSummary> for StoredSystemState {
    fn from(native: IotaSystemStateSummary) -> Self {
        match native {
            IotaSystemStateSummary::V1(inner) => StoredSystemState::V1(inner.into()),
            IotaSystemStateSummary::V2(inner) => StoredSystemState::V2(inner.into()),
            _ => panic!("unsupported native system state"),
        }
    }
}
