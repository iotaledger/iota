// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    proto::timestamp_ms_to_proto,
    read_masks::GET_EPOCH_READ_MASK,
    v1::{
        bcs::BcsData,
        epoch::{Epoch, ProtocolConfig},
        ledger_service::{GetEpochRequest, GetEpochResponse},
    },
};
use iota_protocol_config::{Chain, ProtocolConfig as IotaProtocolConfig};
use iota_types::committee::EpochId;
use tonic::Status;

use crate::{
    error::RpcError, ledger_service::LedgerGrpcService, merge::Merge, types::GrpcReader,
    validation::validate_read_mask,
};

/// Source for building `Epoch` using the `Merge` trait.
pub struct EpochReadSource {
    pub reader: Arc<GrpcReader>,
    pub chain: Chain,
    pub epoch: u64,
    pub current_epoch: u64,
}

impl Merge<&EpochReadSource> for Epoch {
    type Error = RpcError;

    fn merge(&mut self, source: &EpochReadSource, mask: &FieldMaskTree) -> Result<(), Self::Error> {
        if mask.contains(Self::EPOCH_FIELD.name) {
            self.epoch = Some(source.epoch);
        }

        // Fetch epoch_info once for all fields that depend on it.
        let needs_epoch_info = mask.contains(Self::FIRST_CHECKPOINT_FIELD.name)
            || mask.contains(Self::LAST_CHECKPOINT_FIELD.name)
            || mask.contains(Self::START_FIELD.name)
            || mask.contains(Self::END_FIELD.name)
            || mask.contains(Self::REFERENCE_GAS_PRICE_FIELD.name)
            || mask.subtree(Self::PROTOCOL_CONFIG_FIELD.name).is_some()
            || (mask.contains(Self::BCS_SYSTEM_STATE_FIELD.name)
                && source.epoch != source.current_epoch);

        let epoch_info = if needs_epoch_info {
            source
                .reader
                .get_epoch_info(source.epoch)
                .map_err(|e| RpcError::from(e).with_context("failed to get epoch info"))?
        } else {
            None
        };

        if let Some(ref epoch_info) = epoch_info {
            if mask.contains(Self::FIRST_CHECKPOINT_FIELD.name) {
                self.first_checkpoint = Some(epoch_info.start_checkpoint);
            }

            if mask.contains(Self::LAST_CHECKPOINT_FIELD.name) {
                if let Some(end_checkpoint) = epoch_info.end_checkpoint {
                    self.last_checkpoint = Some(end_checkpoint);
                }
            }

            if mask.contains(Self::START_FIELD.name) {
                self.start = Some(timestamp_ms_to_proto(epoch_info.start_timestamp_ms));
            }

            if mask.contains(Self::END_FIELD.name) {
                if let Some(end_timestamp_ms) = epoch_info.end_timestamp_ms {
                    self.end = Some(timestamp_ms_to_proto(end_timestamp_ms));
                }
            }

            if mask.contains(Self::REFERENCE_GAS_PRICE_FIELD.name) {
                self.reference_gas_price = Some(epoch_info.reference_gas_price);
            }

            if let Some(submask) = mask.subtree(Self::PROTOCOL_CONFIG_FIELD.name) {
                let iota_config = IotaProtocolConfig::get_for_version_if_supported(
                    epoch_info.protocol_version.into(),
                    source.chain,
                )
                .ok_or_else(|| ProtocolVersionNotFoundError::new(epoch_info.protocol_version))?;
                self.protocol_config = Some(ProtocolConfig::merge_from(&iota_config, &submask)?);
            }
        }

        if mask.contains(Self::BCS_SYSTEM_STATE_FIELD.name) {
            // For the current epoch use the live system state; for historical
            // epochs use the snapshot stored at the start of the epoch.
            let system_state = if source.epoch == source.current_epoch {
                source
                    .reader
                    .get_system_state()
                    .map_err(|e| RpcError::from(e).with_context("failed to get system state"))?
            } else if let Some(ref info) = epoch_info {
                info.system_state.clone()
            } else {
                return Err(RpcError::internal().with_context(format!(
                    "cannot get system state for historical epoch {}: epoch info not available",
                    source.epoch
                )));
            };
            self.bcs_system_state = Some(BcsData::serialize(&system_state)?);
        }

        if mask.contains(Self::COMMITTEE_FIELD.name) {
            let committee = source
                .reader
                .get_committee(source.epoch)
                .map_err(|e| RpcError::from(e).with_context("failed to get committee"))?
                .ok_or_else(|| CommitteeNotFoundError::new(source.epoch))?;
            let sdk_committee: iota_sdk_types::ValidatorCommittee =
                committee.as_ref().clone().into();
            self.committee = Some(sdk_committee.into());
        }

        Ok(())
    }
}

/// Available Read Mask Fields
///
/// The `get_epoch` function supports the following `read_mask` fields to
/// control which data is included in the response:
///
/// ## Epoch Fields
/// - `epoch` - the epoch number
/// - `committee` - the validator committee for this epoch
/// - `bcs_system_state` - the BCS-encoded system state at the beginning of the
///   epoch for past epochs or the current system state for the current epoch,
///   which can be used for historical state queries or to get the current state
///   respectively
///
/// ## Checkpoint Fields
/// - `first_checkpoint` - the first checkpoint included in the epoch
/// - `last_checkpoint` - the last checkpoint included in the epoch, which may
///   be unavailable for the current epoch if it has not ended yet
///
/// ## Timing Fields
/// - `start` - the timestamp of the first checkpoint included in the epoch
/// - `end` - the timestamp of the last checkpoint included in the epoch, which
///   may be unavailable for the current epoch if it has not ended yet
///
/// ## Gas Fields
/// - `reference_gas_price` - the reference gas price during the epoch,
///   denominated in NANOS
///
/// ## Protocol Configuration Fields
/// - `protocol_config` - the protocol configuration during the epoch
///   - `protocol_config.protocol_version` - the protocol version during the
///     epoch
///   - `protocol_config.feature_flags` - the individual protocol feature flags
///     during the epoch (use `protocol_config.feature_flags.<key>` to filter
///     specific flags)
///   - `protocol_config.attributes` - the individual protocol attributes during
///     the epoch (use `protocol_config.attributes.<key>` to filter specific
///     attributes)
#[tracing::instrument(skip(service))]
pub fn get_epoch(
    service: &LedgerGrpcService,
    request: GetEpochRequest,
) -> Result<GetEpochResponse, Status> {
    let read_mask = validate_read_mask::<Epoch>(request.read_mask, GET_EPOCH_READ_MASK)?;

    let current_epoch = service
        .reader
        .get_latest_checkpoint()
        .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
        .epoch();
    let epoch = request.epoch.unwrap_or(current_epoch);

    let source = EpochReadSource {
        reader: service.reader.clone(),
        chain: service.chain,
        epoch,
        current_epoch,
    };

    let message = Epoch::merge_from(&source, &read_mask)
        .map_err(|e| e.with_context("failed to merge epoch"))?;

    Ok(GetEpochResponse::default().with_epoch(message))
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

impl From<CommitteeNotFoundError> for RpcError {
    fn from(value: CommitteeNotFoundError) -> Self {
        RpcError::not_found().with_context(value)
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

impl From<ProtocolVersionNotFoundError> for RpcError {
    fn from(value: ProtocolVersionNotFoundError) -> Self {
        RpcError::not_found().with_context(value)
    }
}
