// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.epoch.rs");
include!("../../../generated/iota.grpc.v0.epoch.field_info.rs");
include!("../../../generated/iota.grpc.v0.epoch.accessors.rs");

use tap::Pipe;

use crate::{field::FieldMaskTree, merge::Merge, proto::TryFromProtoError};

// ValidatorCommitteeMember
//

impl From<iota_sdk_types::ValidatorCommitteeMember> for ValidatorCommitteeMember {
    fn from(value: iota_sdk_types::ValidatorCommitteeMember) -> Self {
        Self {
            public_key: Some(value.public_key.as_bytes().to_vec().into()),
            weight: Some(value.stake),
        }
    }
}

impl TryFrom<&ValidatorCommitteeMember> for iota_sdk_types::ValidatorCommitteeMember {
    type Error = TryFromProtoError;

    fn try_from(
        ValidatorCommitteeMember { public_key, weight }: &ValidatorCommitteeMember,
    ) -> Result<Self, Self::Error> {
        let public_key = public_key
            .as_ref()
            .ok_or_else(|| {
                TryFromProtoError::missing(ValidatorCommitteeMember::PUBLIC_KEY_FIELD.name)
            })?
            .as_ref()
            .pipe(iota_sdk_types::Bls12381PublicKey::from_bytes)
            .map_err(|e| {
                TryFromProtoError::invalid(ValidatorCommitteeMember::PUBLIC_KEY_FIELD, e)
            })?;

        let stake = weight.ok_or_else(|| {
            TryFromProtoError::missing(ValidatorCommitteeMember::WEIGHT_FIELD.name)
        })?;
        Ok(Self { public_key, stake })
    }
}

// ValidatorCommittee
//

impl From<iota_sdk_types::ValidatorCommittee> for ValidatorCommittee {
    fn from(value: iota_sdk_types::ValidatorCommittee) -> Self {
        Self {
            epoch: Some(value.epoch),
            members: Some(ValidatorCommitteeMembers {
                members: value.members.into_iter().map(Into::into).collect(),
            }),
        }
    }
}

impl TryFrom<&ValidatorCommittee> for iota_sdk_types::ValidatorCommittee {
    type Error = TryFromProtoError;

    fn try_from(value: &ValidatorCommittee) -> Result<Self, Self::Error> {
        let epoch = value
            .epoch
            .ok_or_else(|| TryFromProtoError::missing(ValidatorCommittee::EPOCH_FIELD.name))?;
        let members = value
            .members
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(ValidatorCommittee::MEMBERS_FIELD.name))?;
        Ok(Self {
            epoch,
            members: members
                .members
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl Merge<&Epoch> for Epoch {
    fn merge(
        &mut self,
        source: &Epoch,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Epoch {
            epoch,
            committee,
            bcs_system_state,
            first_checkpoint,
            last_checkpoint,
            start,
            end,
            reference_gas_price,
            protocol_config,
        } = source;

        if mask.contains(Self::EPOCH_FIELD.name) {
            self.epoch = *epoch;
        }

        if mask.contains(Self::COMMITTEE_FIELD.name) {
            self.committee = committee.to_owned();
        }

        if mask.contains(Self::BCS_SYSTEM_STATE_FIELD.name) {
            self.bcs_system_state = bcs_system_state.to_owned();
        }

        if mask.contains(Self::FIRST_CHECKPOINT_FIELD.name) {
            self.first_checkpoint = first_checkpoint.to_owned();
        }

        if mask.contains(Self::LAST_CHECKPOINT_FIELD.name) {
            self.last_checkpoint = last_checkpoint.to_owned();
        }

        if mask.contains(Self::START_FIELD.name) {
            self.start = start.to_owned();
        }

        if mask.contains(Self::END_FIELD.name) {
            self.end = end.to_owned();
        }

        if mask.contains(Self::REFERENCE_GAS_PRICE_FIELD.name) {
            self.reference_gas_price = reference_gas_price.to_owned();
        }

        if let Some(submask) = mask.subtree(Self::PROTOCOL_CONFIG_FIELD.name) {
            self.protocol_config = protocol_config
                .as_ref()
                .map(|config| ProtocolConfig::merge_from(config, &submask))
                .transpose()?;
        }

        Ok(())
    }
}

impl Merge<&ProtocolConfig> for ProtocolConfig {
    fn merge(
        &mut self,
        source: &ProtocolConfig,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ProtocolConfig {
            protocol_version,
            feature_flags,
            attributes,
        } = source;

        if mask.contains(Self::PROTOCOL_VERSION_FIELD.name) {
            self.protocol_version = *protocol_version;
        }

        if mask.contains(Self::FEATURE_FLAGS_FIELD.name) {
            self.feature_flags = feature_flags.to_owned();
        }

        if mask.contains(Self::ATTRIBUTES_FIELD.name) {
            self.attributes = attributes.to_owned();
        }

        Ok(())
    }
}

impl Merge<ProtocolConfig> for ProtocolConfig {
    fn merge(
        &mut self,
        source: ProtocolConfig,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ProtocolConfig {
            protocol_version,
            feature_flags,
            attributes,
        } = source;

        if mask.contains(Self::PROTOCOL_VERSION_FIELD.name) {
            self.protocol_version = protocol_version;
        }

        if mask.contains(Self::FEATURE_FLAGS_FIELD.name) {
            self.feature_flags = feature_flags;
        }

        if mask.contains(Self::ATTRIBUTES_FIELD.name) {
            self.attributes = attributes;
        }

        Ok(())
    }
}

/// Convert iota_types::committee::Committee to protobuf ValidatorCommittee
impl From<&iota_types::committee::Committee> for ValidatorCommittee {
    fn from(committee: &iota_types::committee::Committee) -> Self {
        let members_vec: Vec<ValidatorCommitteeMember> = committee
            .voting_rights
            .iter()
            .map(|(public_key, weight)| ValidatorCommitteeMember {
                public_key: Some(public_key.0.to_vec().into()),
                weight: Some(*weight),
            })
            .collect();

        ValidatorCommittee {
            epoch: Some(committee.epoch),
            members: Some(ValidatorCommitteeMembers {
                members: members_vec,
            }),
        }
    }
}
