// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.epoch.rs");
include!("../../../generated/iota.grpc.v0.epoch.field_info.rs");

use tap::Pipe;

use crate::proto::TryFromProtoError;

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

impl Epoch {
    pub fn committee(&self) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        match &self.committee {
            Some(committee) => Ok(committee.try_into()?),
            None => Err(TryFromProtoError::missing("committee")),
        }
    }
}

impl ValidatorCommittee {
    pub fn validator_committee(
        &self,
    ) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        self.try_into()
    }
}

impl ValidatorCommitteeMember {
    pub fn committee_member(
        &self,
    ) -> Result<iota_sdk_types::ValidatorCommitteeMember, TryFromProtoError> {
        self.try_into()
    }
}
