// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use fastcrypto::traits::ToFromBytes;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use super::{
    AdvanceEpochParams, IotaSystemStateTrait,
    epoch_start_iota_system_state::EpochStartValidatorInfoV1,
    get_validators_from_table_vec,
    iota_system_state_summary::{
        IotaSystemStateSummary, IotaSystemStateSummaryV1, IotaValidatorSummary,
    },
};
use crate::{
    balance::Balance,
    base_types::{IotaAddress, ObjectID},
    collection_types::{Bag, Table, TableVec, VecMap, VecSet},
    committee::{CommitteeWithNetworkMetadata, NetworkMetadata},
    crypto::{
        AuthorityPublicKey, AuthorityPublicKeyBytes, AuthoritySignature, NetworkPublicKey,
        verify_proof_of_possession,
    },
    error::IotaError,
    gas_coin::IotaTreasuryCap,
    id::ID,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemState,
    multiaddr::Multiaddr,
    storage::ObjectStore,
    system_admin_cap::IotaSystemAdminCap,
};

const E_METADATA_INVALID_POP: u64 = 0;
const E_METADATA_INVALID_AUTHORITY_PUBKEY: u64 = 1;
const E_METADATA_INVALID_NET_PUBKEY: u64 = 2;
const E_METADATA_INVALID_PROTOCOL_PUBKEY: u64 = 3;
const E_METADATA_INVALID_NET_ADDR: u64 = 4;
const E_METADATA_INVALID_P2P_ADDR: u64 = 5;
const E_METADATA_INVALID_PRIMARY_ADDR: u64 = 6;

/// Rust version of the Move iota::iota_system::SystemParameters type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SystemParametersV1 {
    /// The duration of an epoch, in milliseconds.
    pub epoch_duration_ms: u64,

    /// Minimum number of active validators at any moment.
    pub min_validator_count: u64,

    /// Maximum number of active validators at any moment.
    /// We do not allow the number of validators in any epoch to go above this.
    pub max_validator_count: u64,

    /// Lower-bound on the amount of stake required to become a validator.
    pub min_validator_joining_stake: u64,

    /// Validators with stake amount below `validator_low_stake_threshold` are
    /// considered to have low stake and will be escorted out of the
    /// validator set after being below this threshold for more than
    /// `validator_low_stake_grace_period` number of epochs.
    pub validator_low_stake_threshold: u64,

    /// Validators with stake below `validator_very_low_stake_threshold` will be
    /// removed immediately at epoch change, no grace period.
    pub validator_very_low_stake_threshold: u64,

    /// A validator can have stake below `validator_low_stake_threshold`
    /// for this many epochs before being kicked out.
    pub validator_low_stake_grace_period: u64,

    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ValidatorMetadataV1 {
    pub iota_address: IotaAddress,
    pub authority_pubkey_bytes: Vec<u8>,
    pub network_pubkey_bytes: Vec<u8>,
    pub protocol_pubkey_bytes: Vec<u8>,
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    pub next_epoch_authority_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    pub next_epoch_network_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_protocol_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_net_address: Option<String>,
    pub next_epoch_p2p_address: Option<String>,
    pub next_epoch_primary_address: Option<String>,
    pub extra_fields: Bag,
}

#[derive(derive_more::Debug, Clone, Eq, PartialEq)]
pub struct VerifiedValidatorMetadataV1 {
    pub iota_address: IotaAddress,
    pub authority_pubkey: AuthorityPublicKey,
    pub network_pubkey: NetworkPublicKey,
    pub protocol_pubkey: NetworkPublicKey,
    #[debug(skip)]
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: Multiaddr,
    pub p2p_address: Multiaddr,
    pub primary_address: Multiaddr,
    pub next_epoch_authority_pubkey: Option<AuthorityPublicKey>,
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    pub next_epoch_network_pubkey: Option<NetworkPublicKey>,
    pub next_epoch_protocol_pubkey: Option<NetworkPublicKey>,
    pub next_epoch_net_address: Option<Multiaddr>,
    pub next_epoch_p2p_address: Option<Multiaddr>,
    pub next_epoch_primary_address: Option<Multiaddr>,
}

impl VerifiedValidatorMetadataV1 {
    pub fn iota_pubkey_bytes(&self) -> AuthorityPublicKeyBytes {
        (&self.authority_pubkey).into()
    }
}

impl ValidatorMetadataV1 {
    /// Verify validator metadata and return a verified version (on success) or
    /// error code (on failure)
    pub fn verify(&self) -> Result<VerifiedValidatorMetadataV1, u64> {
        let authority_pubkey = AuthorityPublicKey::from_bytes(self.authority_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_AUTHORITY_PUBKEY)?;

        // Verify proof of possession for the authority key
        let pop = AuthoritySignature::from_bytes(self.proof_of_possession_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_POP)?;
        verify_proof_of_possession(&pop, &authority_pubkey, self.iota_address)
            .map_err(|_| E_METADATA_INVALID_POP)?;

        let network_pubkey = NetworkPublicKey::from_bytes(self.network_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_NET_PUBKEY)?;
        let protocol_pubkey = NetworkPublicKey::from_bytes(self.protocol_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_PROTOCOL_PUBKEY)?;
        if protocol_pubkey == network_pubkey {
            return Err(E_METADATA_INVALID_PROTOCOL_PUBKEY);
        }

        let net_address = Multiaddr::try_from(self.net_address.clone())
            .map_err(|_| E_METADATA_INVALID_NET_ADDR)?;

        // Ensure p2p and primary address are both Multiaddr's and valid
        // anemo addresses
        let p2p_address = Multiaddr::try_from(self.p2p_address.clone())
            .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;
        p2p_address
            .to_anemo_address()
            .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;

        let primary_address = Multiaddr::try_from(self.primary_address.clone())
            .map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;
        primary_address
            .to_anemo_address()
            .map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;

        let next_epoch_authority_pubkey = match self.next_epoch_authority_pubkey_bytes.clone() {
            None => Ok::<Option<AuthorityPublicKey>, u64>(None),
            Some(bytes) => Ok(Some(
                AuthorityPublicKey::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_AUTHORITY_PUBKEY)?,
            )),
        }?;

        let next_epoch_pop = match self.next_epoch_proof_of_possession.clone() {
            None => Ok::<Option<AuthoritySignature>, u64>(None),
            Some(bytes) => Ok(Some(
                AuthoritySignature::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_POP)?,
            )),
        }?;
        // Verify proof of possession for the next epoch authority key
        if let Some(ref next_epoch_authority_pubkey) = next_epoch_authority_pubkey {
            match next_epoch_pop {
                Some(next_epoch_pop) => {
                    verify_proof_of_possession(
                        &next_epoch_pop,
                        next_epoch_authority_pubkey,
                        self.iota_address,
                    )
                    .map_err(|_| E_METADATA_INVALID_POP)?;
                }
                None => {
                    return Err(E_METADATA_INVALID_POP);
                }
            }
        }

        let next_epoch_network_pubkey = match self.next_epoch_network_pubkey_bytes.clone() {
            None => Ok::<Option<NetworkPublicKey>, u64>(None),
            Some(bytes) => Ok(Some(
                NetworkPublicKey::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_NET_PUBKEY)?,
            )),
        }?;

        let next_epoch_protocol_pubkey: Option<NetworkPublicKey> =
            match self.next_epoch_protocol_pubkey_bytes.clone() {
                None => Ok::<Option<NetworkPublicKey>, u64>(None),
                Some(bytes) => Ok(Some(
                    NetworkPublicKey::from_bytes(bytes.as_ref())
                        .map_err(|_| E_METADATA_INVALID_PROTOCOL_PUBKEY)?,
                )),
            }?;
        if next_epoch_network_pubkey.is_some()
            && next_epoch_network_pubkey == next_epoch_protocol_pubkey
        {
            return Err(E_METADATA_INVALID_PROTOCOL_PUBKEY);
        }

        let next_epoch_net_address = match self.next_epoch_net_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => Ok(Some(
                Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_NET_ADDR)?,
            )),
        }?;

        let next_epoch_p2p_address = match self.next_epoch_p2p_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => {
                let address =
                    Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;
                address
                    .to_anemo_address()
                    .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;

                Ok(Some(address))
            }
        }?;

        let next_epoch_primary_address = match self.next_epoch_primary_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => {
                let address =
                    Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;
                address
                    .to_anemo_address()
                    .map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;

                Ok(Some(address))
            }
        }?;

        Ok(VerifiedValidatorMetadataV1 {
            iota_address: self.iota_address,
            authority_pubkey,
            network_pubkey,
            protocol_pubkey,
            proof_of_possession_bytes: self.proof_of_possession_bytes.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            image_url: self.image_url.clone(),
            project_url: self.project_url.clone(),
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey,
            next_epoch_proof_of_possession: self.next_epoch_proof_of_possession.clone(),
            next_epoch_network_pubkey,
            next_epoch_protocol_pubkey,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
        })
    }
}

/// Rust version of the Move iota::validator::ValidatorV1 type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ValidatorV1 {
    metadata: ValidatorMetadataV1,
    #[serde(skip)]
    verified_metadata: OnceCell<VerifiedValidatorMetadataV1>,

    pub voting_power: u64,
    pub operation_cap_id: ID,
    pub gas_price: u64,
    pub staking_pool: StakingPoolV1,
    pub commission_rate: u64,
    pub next_epoch_stake: u64,
    pub next_epoch_gas_price: u64,
    pub next_epoch_commission_rate: u64,
    pub extra_fields: Bag,
}

impl ValidatorV1 {
    pub fn verified_metadata(&self) -> &VerifiedValidatorMetadataV1 {
        self.verified_metadata.get_or_init(|| {
            self.metadata
                .verify()
                .expect("Validity of metadata should be verified on-chain")
        })
    }

    pub fn into_iota_validator_summary(self) -> IotaValidatorSummary {
        let Self {
            metadata:
                ValidatorMetadataV1 {
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
                    extra_fields: _,
                },
            verified_metadata: _,
            voting_power,
            operation_cap_id,
            gas_price,
            staking_pool:
                StakingPoolV1 {
                    id: staking_pool_id,
                    activation_epoch: staking_pool_activation_epoch,
                    deactivation_epoch: staking_pool_deactivation_epoch,
                    iota_balance: staking_pool_iota_balance,
                    rewards_pool,
                    pool_token_balance,
                    exchange_rates:
                        Table {
                            id: exchange_rates_id,
                            size: exchange_rates_size,
                        },
                    pending_stake,
                    pending_total_iota_withdraw,
                    pending_pool_token_withdraw,
                    extra_fields: _,
                },
            commission_rate,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
            extra_fields: _,
        } = self;
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
            operation_cap_id: operation_cap_id.bytes,
            gas_price,
            staking_pool_id,
            staking_pool_activation_epoch,
            staking_pool_deactivation_epoch,
            staking_pool_iota_balance,
            rewards_pool: rewards_pool.value(),
            pool_token_balance,
            exchange_rates_id,
            exchange_rates_size,
            pending_stake,
            pending_total_iota_withdraw,
            pending_pool_token_withdraw,
            commission_rate,
            next_epoch_stake,
            next_epoch_gas_price,
            next_epoch_commission_rate,
        }
    }
}

/// Rust version of the Move iota_system::staking_pool::StakingPoolV1 type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct StakingPoolV1 {
    pub id: ObjectID,
    pub activation_epoch: Option<u64>,
    pub deactivation_epoch: Option<u64>,
    pub iota_balance: u64,
    pub rewards_pool: Balance,
    pub pool_token_balance: u64,
    pub exchange_rates: Table,
    pub pending_stake: u64,
    pub pending_total_iota_withdraw: u64,
    pub pending_pool_token_withdraw: u64,
    pub extra_fields: Bag,
}

/// Rust version of the Move iota_system::validator_set::ValidatorSetV1 type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ValidatorSetV1 {
    pub total_stake: u64,
    pub active_validators: Vec<ValidatorV1>,
    pub pending_active_validators: TableVec,
    pub pending_removals: Vec<u64>,
    pub staking_pool_mappings: Table,
    pub inactive_validators: Table,
    pub validator_candidates: Table,
    pub at_risk_validators: VecMap<IotaAddress, u64>,
    pub extra_fields: Bag,
}

/// Rust version of the Move iota_system::storage_fund::StorageFundV1 type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct StorageFundV1 {
    pub total_object_storage_rebates: Balance,
    pub non_refundable_balance: Balance,
}

/// Rust version of the Move iota_system::iota_system::IotaSystemStateV1 type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct IotaSystemStateV1 {
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
    pub safe_mode_computation_rewards: Balance,
    pub safe_mode_storage_rebates: u64,
    pub safe_mode_non_refundable_storage_fee: u64,
    pub epoch_start_timestamp_ms: u64,
    pub extra_fields: Bag,
    // TODO: Use getters instead of all pub.
}

impl IotaSystemStateTrait for IotaSystemStateV1 {
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
        self.safe_mode_computation_rewards
            .deposit_for_safe_mode(params.computation_charge);
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
            safe_mode_computation_rewards,
            safe_mode_storage_rebates,
            safe_mode_non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            extra_fields: _,
        } = self;
        IotaSystemStateSummary::V1(IotaSystemStateSummaryV1 {
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
            safe_mode_computation_rewards: safe_mode_computation_rewards.value(),
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

/// Rust version of the Move
/// iota_system::validator_cap::UnverifiedValidatorOperationCap type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct UnverifiedValidatorOperationCap {
    pub id: ObjectID,
    pub authorizer_validator_address: IotaAddress,
}
