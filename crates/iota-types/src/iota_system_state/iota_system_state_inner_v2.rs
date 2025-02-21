// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{
    AdvanceEpochParams, IotaSystemStateTrait,
    epoch_start_iota_system_state::EpochStartValidatorInfoV1,
    get_validators_from_table_vec,
    iota_system_state_inner_v1::{StorageFundV1, SystemParametersV1, ValidatorSetV1, ValidatorV1},
    iota_system_state_summary::{
        IotaSystemStateSummary, IotaSystemStateSummaryV2, IotaValidatorSummary,
    },
};
use crate::{
    balance::Balance,
    base_types::IotaAddress,
    collection_types::{Bag, Table, TableVec, VecMap, VecSet},
    committee::{CommitteeWithNetworkMetadata, NetworkMetadata},
    error::IotaError,
    gas_coin::IotaTreasuryCap,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemState,
    storage::ObjectStore,
    system_admin_cap::IotaSystemAdminCap,
};

/// Rust version of the Move
/// `iota_framework::iota_system::iota_system::IotaSystemStateV2`
/// type. An additional field `safe_mode_computation_charges_burned` is added
/// over `IotaSystemStateV2` to allow for burning of base fees in safe mode when
/// protocol_defined_base_fee is enabled in the protocol config.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct IotaSystemStateV2 {
    pub epoch: u64,
    pub protocol_version: u64,
    pub system_state_version: u64,
    pub iota_treasury_cap: IotaTreasuryCap,
    pub validators: ValidatorSetV1,
    pub storage_fund: StorageFundV1,
    pub parameters: SystemParametersV1,
    pub iota_system_admin_cap: IotaSystemAdminCap,
    pub reference_gas_price: u64,
    pub validator_report_records: VecMap<IotaAddress, VecSet<IotaAddress>>,
    pub safe_mode: bool,
    pub safe_mode_storage_charges: Balance,
    pub safe_mode_computation_charges: Balance,
    pub safe_mode_computation_charges_burned: u64,
    pub safe_mode_storage_rebates: u64,
    pub safe_mode_non_refundable_storage_fee: u64,
    pub epoch_start_timestamp_ms: u64,
    pub extra_fields: Bag,
    // TODO: Use getters instead of all pub.
}

impl IotaSystemStateTrait for IotaSystemStateV2 {
    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn reference_gas_price(&self) -> u64 {
        self.reference_gas_price
    }

    fn protocol_version(&self) -> u64 {
        self.protocol_version
    }

    fn system_state_version(&self) -> u64 {
        self.system_state_version
    }

    fn epoch_start_timestamp_ms(&self) -> u64 {
        self.epoch_start_timestamp_ms
    }

    fn epoch_duration_ms(&self) -> u64 {
        self.parameters.epoch_duration_ms
    }

    fn safe_mode(&self) -> bool {
        self.safe_mode
    }

    fn advance_epoch_safe_mode(&mut self, params: &AdvanceEpochParams) {
        self.epoch = params.epoch;
        self.safe_mode = true;
        self.safe_mode_storage_charges
            .deposit_for_safe_mode(params.storage_charge);
        self.safe_mode_storage_rebates += params.storage_rebate;
        self.safe_mode_computation_charges
            .deposit_for_safe_mode(params.computation_charge);
        self.safe_mode_computation_charges_burned += params.computation_charge_burned;
        self.safe_mode_non_refundable_storage_fee += params.non_refundable_storage_fee;
        self.epoch_start_timestamp_ms = params.epoch_start_timestamp_ms;
        self.protocol_version = params.next_protocol_version.as_u64();
    }

    fn get_current_epoch_committee(&self) -> CommitteeWithNetworkMetadata {
        let validators = self
            .validators
            .active_validators
            .iter()
            .map(|validator| {
                let verified_metadata = validator.verified_metadata();
                let name = verified_metadata.iota_pubkey_bytes();
                (
                    name,
                    (
                        validator.voting_power,
                        NetworkMetadata {
                            network_address: verified_metadata.net_address.clone(),
                            primary_address: verified_metadata.primary_address.clone(),
                        },
                    ),
                )
            })
            .collect();
        CommitteeWithNetworkMetadata::new(self.epoch, validators)
    }

    fn get_pending_active_validators<S: ObjectStore + ?Sized>(
        &self,
        object_store: &S,
    ) -> Result<Vec<IotaValidatorSummary>, IotaError> {
        let table_id = self.validators.pending_active_validators.contents.id;
        let table_size = self.validators.pending_active_validators.contents.size;
        let validators: Vec<ValidatorV1> =
            get_validators_from_table_vec(&object_store, table_id, table_size)?;
        Ok(validators
            .into_iter()
            .map(|v| v.into_iota_validator_summary())
            .collect())
    }

    fn into_epoch_start_state(self) -> EpochStartSystemState {
        EpochStartSystemState::new_v1(
            self.epoch,
            self.protocol_version,
            self.reference_gas_price,
            self.safe_mode,
            self.epoch_start_timestamp_ms,
            self.parameters.epoch_duration_ms,
            self.validators
                .active_validators
                .iter()
                .map(|validator| {
                    let metadata = validator.verified_metadata();
                    EpochStartValidatorInfoV1 {
                        iota_address: metadata.iota_address,
                        authority_pubkey: metadata.authority_pubkey.clone(),
                        network_pubkey: metadata.network_pubkey.clone(),
                        protocol_pubkey: metadata.protocol_pubkey.clone(),
                        iota_net_address: metadata.net_address.clone(),
                        p2p_address: metadata.p2p_address.clone(),
                        primary_address: metadata.primary_address.clone(),
                        voting_power: validator.voting_power,
                        hostname: metadata.name.clone(),
                    }
                })
                .collect(),
        )
    }

    fn into_iota_system_state_summary(self) -> IotaSystemStateSummary {
        let Self {
            epoch,
            protocol_version,
            system_state_version,
            iota_treasury_cap,
            validators:
                ValidatorSetV1 {
                    total_stake,
                    active_validators,
                    pending_active_validators:
                        TableVec {
                            contents:
                                Table {
                                    id: pending_active_validators_id,
                                    size: pending_active_validators_size,
                                },
                        },
                    pending_removals,
                    staking_pool_mappings:
                        Table {
                            id: staking_pool_mappings_id,
                            size: staking_pool_mappings_size,
                        },
                    inactive_validators:
                        Table {
                            id: inactive_pools_id,
                            size: inactive_pools_size,
                        },
                    validator_candidates:
                        Table {
                            id: validator_candidates_id,
                            size: validator_candidates_size,
                        },
                    at_risk_validators:
                        VecMap {
                            contents: at_risk_validators,
                        },
                    extra_fields: _,
                },
            storage_fund,
            parameters:
                SystemParametersV1 {
                    epoch_duration_ms,
                    min_validator_count,
                    max_validator_count,
                    min_validator_joining_stake,
                    validator_low_stake_threshold,
                    validator_very_low_stake_threshold,
                    validator_low_stake_grace_period,
                    extra_fields: _,
                },
            iota_system_admin_cap: _,
            reference_gas_price,
            validator_report_records:
                VecMap {
                    contents: validator_report_records,
                },
            safe_mode,
            safe_mode_storage_charges,
            safe_mode_computation_charges,
            safe_mode_computation_charges_burned,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            extra_fields: _,
        } = self;
        IotaSystemStateSummary::V2(IotaSystemStateSummaryV2 {
            epoch,
            protocol_version,
            system_state_version,
            iota_total_supply: iota_treasury_cap.total_supply().value,
            iota_treasury_cap_id: iota_treasury_cap.id().to_owned(),
            storage_fund_total_object_storage_rebates: storage_fund
                .total_object_storage_rebates
                .value(),
            storage_fund_non_refundable_balance: storage_fund.non_refundable_balance.value(),
            reference_gas_price,
            safe_mode,
            safe_mode_storage_charges: safe_mode_storage_charges.value(),
            safe_mode_computation_charges: safe_mode_computation_charges.value(),
            safe_mode_computation_charges_burned,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            total_stake,
            active_validators: active_validators
                .into_iter()
                .map(|v| v.into_iota_validator_summary())
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
            at_risk_validators: at_risk_validators
                .into_iter()
                .map(|e| (e.key, e.value))
                .collect(),
            validator_report_records: validator_report_records
                .into_iter()
                .map(|e| (e.key, e.value.contents))
                .collect(),
            min_validator_count,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
        })
    }
}
