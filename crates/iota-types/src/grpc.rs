// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::epoch::{
    ValidatorCommittee as GrpcValidatorCommittee,
    ValidatorCommitteeMember as GrpcValidatorCommitteeMember,
    ValidatorCommitteeMembers as GrpcValidatorCommitteeMembers,
};

use crate::committee::Committee;

impl From<&Committee> for GrpcValidatorCommittee {
    fn from(committee: &Committee) -> Self {
        let members_vec: Vec<GrpcValidatorCommitteeMember> = committee
            .voting_rights
            .iter()
            .map(|(public_key, weight)| GrpcValidatorCommitteeMember {
                public_key: Some(public_key.0.to_vec().into()),
                weight: Some(*weight),
            })
            .collect();

        GrpcValidatorCommittee {
            epoch: Some(committee.epoch),
            members: Some(GrpcValidatorCommitteeMembers {
                members: members_vec,
            }),
        }
    }
}
