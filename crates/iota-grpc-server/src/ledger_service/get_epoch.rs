// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    merge::Merge,
    proto::timestamp_ms_to_proto,
    v0::{
        bcs::BcsData,
        epoch::{Epoch, ProtocolConfig},
        ledger_service::{GetEpochRequest, GetEpochResponse},
    },
};
use iota_protocol_config::{Chain, ProtocolConfig as IotaProtocolConfig, ProtocolConfigValue};
use iota_types::committee::EpochId;
use prost_types::FieldMask;
use tonic::Status;

use crate::ledger_service::LedgerGrpcService;

pub const READ_MASK_DEFAULT: &str = crate::field_mask!(
    "epoch",
    "first_checkpoint",
    "last_checkpoint",
    "start",
    "end",
    "reference_gas_price",
    "protocol_config.protocol_version",
);

#[tracing::instrument(skip(service))]
pub fn get_epoch(
    service: &LedgerGrpcService,
    request: GetEpochRequest,
) -> Result<GetEpochResponse, Status> {
    let read_mask = {
        let read_mask = request
            .read_mask
            .unwrap_or_else(|| FieldMask::from_str(READ_MASK_DEFAULT));
        read_mask
            .validate::<Epoch>()
            .map_err(|path| Status::invalid_argument(format!("invalid read_mask path: {path}")))?;
        FieldMaskTree::from(read_mask)
    };

    let mut message = Epoch::default();

    let current_epoch = service
        .reader
        .get_latest_checkpoint()
        .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
        .epoch();
    let epoch = request.epoch.unwrap_or(current_epoch);

    let mut system_state =
        if epoch == current_epoch && read_mask.contains(Epoch::BCS_SYSTEM_STATE_FIELD.name) {
            Some(
                service
                    .reader
                    .get_system_state()
                    .map_err(|e| Status::internal(format!("Failed to get system state: {e}")))?,
            )
        } else {
            None
        };

    if read_mask.contains(Epoch::EPOCH_FIELD.name) {
        message.epoch = Some(epoch);
    }

    if let Some(epoch_info) = service.reader.get_epoch_info(epoch) {
        if read_mask.contains(Epoch::FIRST_CHECKPOINT_FIELD.name) {
            message.first_checkpoint = Some(epoch_info.start_checkpoint);
        }

        if read_mask.contains(Epoch::LAST_CHECKPOINT_FIELD.name) {
            message.last_checkpoint = epoch_info.end_checkpoint;
        }

        if read_mask.contains(Epoch::START_FIELD.name) {
            message.start = Some(timestamp_ms_to_proto(epoch_info.start_timestamp_ms));
        }

        if read_mask.contains(Epoch::END_FIELD.name) {
            message.end = epoch_info.end_timestamp_ms.map(timestamp_ms_to_proto);
        }

        if read_mask.contains(Epoch::REFERENCE_GAS_PRICE_FIELD.name) {
            message.reference_gas_price = Some(epoch_info.reference_gas_price);
        }

        if let Some(submask) = read_mask.subtree(Epoch::PROTOCOL_CONFIG_FIELD.name) {
            message.protocol_config = Some(ProtocolConfig::merge_from(
                get_protocol_config(epoch_info.protocol_version, service.chain)?,
                &submask,
            ));
        }

        // If we're not loading the current epoch then grab the indexed snapshot of the
        // system state at the start of the epoch.
        if system_state.is_none() {
            system_state = Some(epoch_info.system_state);
        }
    }

    if let Some(system_state) = system_state {
        if read_mask.contains(Epoch::BCS_SYSTEM_STATE_FIELD.name) {
            let bcs_bytes = bcs::to_bytes(&system_state).map_err(|e| {
                Status::internal(format!("Failed to serialize system state to BCS: {e}"))
            })?;
            message.bcs_system_state = Some(Box::new(BcsData {
                data: bcs_bytes.into(),
            }));
        }
    }

    if read_mask.contains(Epoch::COMMITTEE_FIELD.name) {
        message.committee = Some(
            service
                .reader
                .get_committee(epoch)
                .map_err(|e| Status::internal(format!("Failed to get committee: {e}")))?
                .ok_or_else(|| CommitteeNotFoundError::new(epoch))?
                .as_ref()
                .into(),
        );
    }

    Ok(GetEpochResponse {
        epoch: Some(message),
    })
}

#[derive(Debug)]
pub struct CommitteeNotFoundError {
    epoch: EpochId,
}

impl CommitteeNotFoundError {
    pub fn new(epoch: EpochId) -> Self {
        Self { epoch }
    }
}

impl std::fmt::Display for CommitteeNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Committee for epoch {} not found", self.epoch)
    }
}

impl std::error::Error for CommitteeNotFoundError {}

impl From<CommitteeNotFoundError> for Status {
    fn from(value: CommitteeNotFoundError) -> Self {
        Status::not_found(value.to_string())
    }
}

#[derive(Debug)]
struct ProtocolVersionNotFoundError {
    version: u64,
}

impl ProtocolVersionNotFoundError {
    pub fn new(version: u64) -> Self {
        Self { version }
    }
}

impl std::fmt::Display for ProtocolVersionNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Protocol version {} not found", self.version)
    }
}

impl std::error::Error for ProtocolVersionNotFoundError {}

impl From<ProtocolVersionNotFoundError> for Status {
    fn from(value: ProtocolVersionNotFoundError) -> Self {
        Status::not_found(value.to_string())
    }
}

fn get_protocol_config(
    version: u64,
    chain: Chain,
) -> std::result::Result<ProtocolConfig, ProtocolVersionNotFoundError> {
    let config = IotaProtocolConfig::get_for_version_if_supported(version.into(), chain)
        .ok_or_else(|| ProtocolVersionNotFoundError::new(version))?;
    Ok(protocol_config_to_proto(config))
}

pub fn protocol_config_to_proto(config: IotaProtocolConfig) -> ProtocolConfig {
    let protocol_version = config.version.as_u64();
    let attributes = config
        .attr_map()
        .into_iter()
        .filter_map(|(k, maybe_v)| {
            maybe_v.map(move |v| {
                let v = match v {
                    ProtocolConfigValue::u16(x) => x.to_string(),
                    ProtocolConfigValue::u32(y) => y.to_string(),
                    ProtocolConfigValue::u64(z) => z.to_string(),
                    ProtocolConfigValue::bool(b) => b.to_string(),
                };
                (k, v)
            })
        })
        .collect();
    let feature_flags = config.feature_map().into_iter().collect();
    ProtocolConfig {
        protocol_version: Some(protocol_version),
        feature_flags: Some(iota_grpc_types::v0::epoch::ProtocolFeatureFlags {
            flags: feature_flags,
        }),
        attributes: Some(iota_grpc_types::v0::epoch::ProtocolAttributes { attributes }),
    }
}
