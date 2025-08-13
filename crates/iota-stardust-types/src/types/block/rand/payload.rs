// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{
    payload::{
        Payload,
        milestone::{MilestoneEssence, MilestoneOptions, MilestonePayload},
        tagged_data::TaggedDataPayload,
        treasury_transaction::TreasuryTransactionPayload,
    },
    rand::{
        bytes::rand_bytes,
        input::rand_treasury_input,
        milestone::{rand_merkle_root, rand_milestone_id, rand_milestone_index},
        number::{rand_number, rand_number_range},
        output::rand_treasury_output,
        parents::rand_parents,
        signature::rand_signature,
    },
};

/// Generates a random tagged data payload.
pub fn rand_tagged_data_payload() -> TaggedDataPayload {
    TaggedDataPayload::new(
        rand_bytes(rand_number_range(TaggedDataPayload::TAG_LENGTH_RANGE).into()),
        rand_bytes(rand_number_range(0..10000)),
    )
    .unwrap()
}

/// Generates a random treasury transaction payload.
pub fn rand_treasury_transaction_payload(token_supply: u64) -> TreasuryTransactionPayload {
    TreasuryTransactionPayload::new(rand_treasury_input(), rand_treasury_output(token_supply))
        .unwrap()
}

/// Generates a random milestone payload.
pub fn rand_milestone_payload(protocol_version: u8) -> MilestonePayload {
    let essence = MilestoneEssence::new(
        rand_milestone_index(),
        rand_number(),
        protocol_version,
        rand_milestone_id(),
        rand_parents(),
        rand_merkle_root(),
        rand_merkle_root(),
        rand_bytes(32),
        MilestoneOptions::from_vec(Vec::new()).unwrap(),
    )
    .unwrap();
    let signatures = [rand_signature()];

    MilestonePayload::new(essence, signatures).unwrap()
}

/// Generates a random payload for a block.
pub fn rand_payload_for_block() -> Payload {
    // TODO complete
    rand_tagged_data_payload().into()
}
