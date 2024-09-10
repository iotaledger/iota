// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::ToFromBytes;
use iota_network_stack::Multiaddr;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::{
    balance::Balance,
    base_types::IotaAddress,
    collection_types::{Bag, Table},
    committee::{Committee, CommitteeWithNetworkMetadata, NetworkMetadata},
    crypto::AuthorityPublicKeyBytes,
    error::IotaError,
    iota_system_state::{
        epoch_start_iota_system_state::{EpochStartSystemState, EpochStartValidatorInfoV1},
        iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
        AdvanceEpochParams, IotaSystemStateTrait,
    },
    storage::ObjectStore,
};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestIotaSystemStateInnerV1 {
    pub epoch: u64,
    pub protocol_version: u64,
    pub system_state_version: u64,
    pub validators: SimTestValidatorSetV1,
    pub storage_fund: Balance,
    pub parameters: SimTestSystemParametersV1,
    pub reference_gas_price: u64,
    pub safe_mode: bool,
    pub epoch_start_timestamp_ms: u64,
    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestSystemParametersV1 {
    pub epoch_duration_ms: u64,
    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestValidatorSetV1 {
    pub active_validators: Vec<SimTestValidatorV1>,
    pub inactive_validators: Table,
    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestValidatorV1 {
    metadata: SimTestValidatorMetadataV1,
    #[serde(skip)]
    verified_metadata: OnceCell<VerifiedSimTestValidatorMetadataV1>,
    pub voting_power: u64,
    pub stake: Balance,
    pub extra_fields: Bag,
}

impl SimTestValidatorV1 {
    pub fn verified_metadata(&self) -> &VerifiedSimTestValidatorMetadataV1 {
        self.verified_metadata
            .get_or_init(|| self.metadata.verify())
    }

    pub fn into_iota_validator_summary(self) -> IotaValidatorSummary {
        IotaValidatorSummary::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestValidatorMetadataV1 {
    pub iota_address: IotaAddress,
    pub protocol_pubkey_bytes: Vec<u8>,
    pub network_pubkey_bytes: Vec<u8>,
    pub worker_pubkey_bytes: Vec<u8>,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    pub worker_address: String,
    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct VerifiedSimTestValidatorMetadataV1 {
    pub iota_address: IotaAddress,
    pub protocol_pubkey: narwhal_crypto::PublicKey,
    pub network_pubkey: narwhal_crypto::NetworkPublicKey,
    pub worker_pubkey: narwhal_crypto::NetworkPublicKey,
    pub net_address: Multiaddr,
    pub p2p_address: Multiaddr,
    pub primary_address: Multiaddr,
    pub worker_address: Multiaddr,
}

impl SimTestValidatorMetadataV1 {
    pub fn verify(&self) -> VerifiedSimTestValidatorMetadataV1 {
        let protocol_pubkey =
            narwhal_crypto::PublicKey::from_bytes(self.protocol_pubkey_bytes.as_ref()).unwrap();
        let network_pubkey =
            narwhal_crypto::NetworkPublicKey::from_bytes(self.network_pubkey_bytes.as_ref())
                .unwrap();
        let worker_pubkey =
            narwhal_crypto::NetworkPublicKey::from_bytes(self.worker_pubkey_bytes.as_ref())
                .unwrap();
        let net_address = Multiaddr::try_from(self.net_address.clone()).unwrap();
        let p2p_address = Multiaddr::try_from(self.p2p_address.clone()).unwrap();
        let primary_address = Multiaddr::try_from(self.primary_address.clone()).unwrap();
        let worker_address = Multiaddr::try_from(self.worker_address.clone()).unwrap();
        VerifiedSimTestValidatorMetadataV1 {
            iota_address: self.iota_address,
            protocol_pubkey,
            network_pubkey,
            worker_pubkey,
            net_address,
            p2p_address,
            primary_address,
            worker_address,
        }
    }
}

impl VerifiedSimTestValidatorMetadataV1 {
    pub fn iota_pubkey_bytes(&self) -> AuthorityPublicKeyBytes {
        (&self.protocol_pubkey).into()
    }
}

impl IotaSystemStateTrait for SimTestIotaSystemStateInnerV1 {
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
        self.epoch_start_timestamp_ms = params.epoch_start_timestamp_ms;
        self.protocol_version = params.next_protocol_version.as_u64();
    }

    fn get_current_epoch_committee(&self) -> CommitteeWithNetworkMetadata {
        let mut voting_rights = BTreeMap::new();
        let mut network_metadata = BTreeMap::new();
        for validator in &self.validators.active_validators {
            let verified_metadata = validator.verified_metadata();
            let name = verified_metadata.iota_pubkey_bytes();
            voting_rights.insert(name, validator.voting_power);
            network_metadata.insert(
                name,
                NetworkMetadata {
                    network_address: verified_metadata.net_address.clone(),
                    narwhal_primary_address: verified_metadata.primary_address.clone(),
                },
            );
        }
        CommitteeWithNetworkMetadata {
            committee: Committee::new(self.epoch, voting_rights),
            network_metadata,
        }
    }

    fn get_pending_active_validators<S: ObjectStore + ?Sized>(
        &self,
        _object_store: &S,
    ) -> Result<Vec<IotaValidatorSummary>, IotaError> {
        Ok(vec![])
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
                        protocol_pubkey: metadata.protocol_pubkey.clone(),
                        narwhal_network_pubkey: metadata.network_pubkey.clone(),
                        narwhal_worker_pubkey: metadata.worker_pubkey.clone(),
                        iota_net_address: metadata.net_address.clone(),
                        p2p_address: metadata.p2p_address.clone(),
                        narwhal_primary_address: metadata.primary_address.clone(),
                        narwhal_worker_address: metadata.worker_address.clone(),
                        voting_power: validator.voting_power,
                        hostname: "".to_string(),
                    }
                })
                .collect(),
        )
    }

    fn into_iota_system_state_summary(self) -> IotaSystemStateSummary {
        IotaSystemStateSummary::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestIotaSystemStateInnerShallowV2 {
    pub new_dummy_field: u64,
    pub epoch: u64,
    pub protocol_version: u64,
    pub system_state_version: u64,
    pub validators: SimTestValidatorSetV1,
    pub storage_fund: Balance,
    pub parameters: SimTestSystemParametersV1,
    pub reference_gas_price: u64,
    pub safe_mode: bool,
    pub epoch_start_timestamp_ms: u64,
    pub extra_fields: Bag,
}

impl IotaSystemStateTrait for SimTestIotaSystemStateInnerShallowV2 {
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
        self.epoch_start_timestamp_ms = params.epoch_start_timestamp_ms;
        self.protocol_version = params.next_protocol_version.as_u64();
    }

    fn get_current_epoch_committee(&self) -> CommitteeWithNetworkMetadata {
        let mut voting_rights = BTreeMap::new();
        let mut network_metadata = BTreeMap::new();
        for validator in &self.validators.active_validators {
            let verified_metadata = validator.verified_metadata();
            let name = verified_metadata.iota_pubkey_bytes();
            voting_rights.insert(name, validator.voting_power);
            network_metadata.insert(
                name,
                NetworkMetadata {
                    network_address: verified_metadata.net_address.clone(),
                    narwhal_primary_address: verified_metadata.primary_address.clone(),
                },
            );
        }
        CommitteeWithNetworkMetadata {
            committee: Committee::new(self.epoch, voting_rights),
            network_metadata,
        }
    }

    fn get_pending_active_validators<S: ObjectStore + ?Sized>(
        &self,
        _object_store: &S,
    ) -> Result<Vec<IotaValidatorSummary>, IotaError> {
        Ok(vec![])
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
                        protocol_pubkey: metadata.protocol_pubkey.clone(),
                        narwhal_network_pubkey: metadata.network_pubkey.clone(),
                        narwhal_worker_pubkey: metadata.worker_pubkey.clone(),
                        iota_net_address: metadata.net_address.clone(),
                        p2p_address: metadata.p2p_address.clone(),
                        narwhal_primary_address: metadata.primary_address.clone(),
                        narwhal_worker_address: metadata.worker_address.clone(),
                        voting_power: validator.voting_power,
                        hostname: "".to_string(),
                    }
                })
                .collect(),
        )
    }

    fn into_iota_system_state_summary(self) -> IotaSystemStateSummary {
        IotaSystemStateSummary::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestValidatorSetDeepV2 {
    pub active_validators: Vec<SimTestValidatorDeepV2>,
    pub inactive_validators: Table,
    pub extra_fields: Bag,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestValidatorDeepV2 {
    pub new_dummy_field: u64,
    metadata: SimTestValidatorMetadataV1,
    #[serde(skip)]
    verified_metadata: OnceCell<VerifiedSimTestValidatorMetadataV1>,
    pub voting_power: u64,
    pub stake: Balance,
    pub extra_fields: Bag,
}

impl SimTestValidatorDeepV2 {
    pub fn verified_metadata(&self) -> &VerifiedSimTestValidatorMetadataV1 {
        self.verified_metadata
            .get_or_init(|| self.metadata.verify())
    }

    pub fn into_iota_validator_summary(self) -> IotaValidatorSummary {
        IotaValidatorSummary::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SimTestIotaSystemStateInnerDeepV2 {
    pub new_dummy_field: u64,
    pub epoch: u64,
    pub protocol_version: u64,
    pub system_state_version: u64,
    pub validators: SimTestValidatorSetDeepV2,
    pub storage_fund: Balance,
    pub parameters: SimTestSystemParametersV1,
    pub reference_gas_price: u64,
    pub safe_mode: bool,
    pub epoch_start_timestamp_ms: u64,
    pub extra_fields: Bag,
}

impl IotaSystemStateTrait for SimTestIotaSystemStateInnerDeepV2 {
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
        self.epoch_start_timestamp_ms = params.epoch_start_timestamp_ms;
        self.protocol_version = params.next_protocol_version.as_u64();
    }

    fn get_current_epoch_committee(&self) -> CommitteeWithNetworkMetadata {
        let mut voting_rights = BTreeMap::new();
        let mut network_metadata = BTreeMap::new();
        for validator in &self.validators.active_validators {
            let verified_metadata = validator.verified_metadata();
            let name = verified_metadata.iota_pubkey_bytes();
            voting_rights.insert(name, validator.voting_power);
            network_metadata.insert(
                name,
                NetworkMetadata {
                    network_address: verified_metadata.net_address.clone(),
                    narwhal_primary_address: verified_metadata.primary_address.clone(),
                },
            );
        }
        CommitteeWithNetworkMetadata {
            committee: Committee::new(self.epoch, voting_rights),
            network_metadata,
        }
    }

    fn get_pending_active_validators<S: ObjectStore + ?Sized>(
        &self,
        _object_store: &S,
    ) -> Result<Vec<IotaValidatorSummary>, IotaError> {
        Ok(vec![])
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
                        protocol_pubkey: metadata.protocol_pubkey.clone(),
                        narwhal_network_pubkey: metadata.network_pubkey.clone(),
                        narwhal_worker_pubkey: metadata.worker_pubkey.clone(),
                        iota_net_address: metadata.net_address.clone(),
                        p2p_address: metadata.p2p_address.clone(),
                        narwhal_primary_address: metadata.primary_address.clone(),
                        narwhal_worker_address: metadata.worker_address.clone(),
                        voting_power: validator.voting_power,
                        hostname: "".to_string(),
                    }
                })
                .collect(),
        )
    }

    fn into_iota_system_state_summary(self) -> IotaSystemStateSummary {
        IotaSystemStateSummary::default()
    }
}
