// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_stardust_types::types::block::{
    Error,
    parent::Parents,
    payload::milestone::{MilestoneEssence, MilestoneIndex, MilestoneOptions, MilestonePayload},
    protocol::protocol_parameters,
    rand::{
        block::rand_block_ids,
        milestone::{rand_merkle_root, rand_milestone_id, rand_milestone_index},
        number::rand_number,
        parents::rand_parents,
        signature::rand_signature,
    },
};
use packable::{PackableExt, bounded::TryIntoBoundedU8Error};
use pretty_assertions::assert_eq;

#[test]
fn kind() {
    assert_eq!(MilestonePayload::KIND, 7);
}

#[test]
fn new_valid() {
    assert!(
        MilestonePayload::new(
            MilestoneEssence::new(
                MilestoneIndex(0),
                0,
                protocol_parameters().protocol_version(),
                rand_milestone_id(),
                rand_parents(),
                rand_merkle_root(),
                rand_merkle_root(),
                [],
                MilestoneOptions::from_vec(vec![]).unwrap(),
            )
            .unwrap(),
            [rand_signature()]
        )
        .is_ok()
    );
}

#[test]
fn new_invalid_no_signature() {
    assert!(matches!(
        MilestonePayload::new(
            MilestoneEssence::new(
                MilestoneIndex(0),
                0,
                protocol_parameters().protocol_version(),
                rand_milestone_id(),
                rand_parents(),
                rand_merkle_root(),
                rand_merkle_root(),
                [],
                MilestoneOptions::from_vec(vec![]).unwrap(),
            )
            .unwrap(),
            []
        ),
        Err(Error::MilestoneInvalidSignatureCount(
            TryIntoBoundedU8Error::Invalid(0)
        ))
    ));
}

#[test]
fn new_invalid_too_many_signatures() {
    assert!(matches!(
        MilestonePayload::new(
            MilestoneEssence::new(
                MilestoneIndex(0),
                0,
                protocol_parameters().protocol_version(),
                rand_milestone_id(),
                rand_parents(),
                rand_merkle_root(),
                rand_merkle_root(),
                [],
                MilestoneOptions::from_vec(vec![]).unwrap(),
            )
            .unwrap(),
            vec![rand_signature(); 300]
        ),
        Err(Error::MilestoneInvalidSignatureCount(
            TryIntoBoundedU8Error::Truncated(300)
        ))
    ));
}

#[test]
fn packed_len() {
    let ms = MilestonePayload::new(
        MilestoneEssence::new(
            MilestoneIndex(0),
            0,
            protocol_parameters().protocol_version(),
            rand_milestone_id(),
            Parents::from_vec(rand_block_ids(4)).unwrap(),
            rand_merkle_root(),
            rand_merkle_root(),
            [0x2a, 0x2a, 0x2a, 0x2a, 0x2a],
            MilestoneOptions::from_vec(vec![]).unwrap(),
        )
        .unwrap(),
        [rand_signature(), rand_signature()],
    )
    .unwrap();

    assert_eq!(ms.packed_len(), 437);
    assert_eq!(ms.pack_to_vec().len(), 437);
}

#[test]
fn pack_unpack_valid() {
    let protocol_parameters = protocol_parameters();
    let payload = MilestonePayload::new(
        MilestoneEssence::new(
            MilestoneIndex(0),
            0,
            protocol_parameters.protocol_version(),
            rand_milestone_id(),
            rand_parents(),
            rand_merkle_root(),
            rand_merkle_root(),
            [],
            MilestoneOptions::from_vec(vec![]).unwrap(),
        )
        .unwrap(),
        [rand_signature()],
    )
    .unwrap();

    let packed = payload.pack_to_vec();

    assert_eq!(payload.packed_len(), packed.len());
    assert_eq!(
        payload,
        PackableExt::unpack_verified(packed.as_slice(), &protocol_parameters).unwrap()
    )
}

#[test]
fn getters() {
    let essence = MilestoneEssence::new(
        rand_milestone_index(),
        rand_number::<u32>(),
        protocol_parameters().protocol_version(),
        rand_milestone_id(),
        rand_parents(),
        rand_merkle_root(),
        rand_merkle_root(),
        [],
        MilestoneOptions::from_vec(vec![]).unwrap(),
    )
    .unwrap();
    let signatures = [rand_signature()];
    let milestone = MilestonePayload::new(essence.clone(), signatures.clone()).unwrap();

    assert_eq!(essence, *milestone.essence());

    assert_eq!(signatures.len(), milestone.signatures().len());
    for (s1, s2) in signatures.iter().zip(milestone.signatures()) {
        assert_eq!(s1, s2);
    }
}
