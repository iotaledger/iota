// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{unlock_condition::AddressUnlockCondition, BasicOutputBuilder, Output},
    payload::transaction::TransactionId,
};

use crate::stardust::types::output_header::OutputHeader;

pub(crate) fn outputs() -> Vec<(OutputHeader, Output)> {
    let mut outputs = Vec::new();

    let output_header = OutputHeader::new_testing(
        *TransactionId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb1812345678",
        )
        .unwrap(),
        rand::random(),
        rand::random(),
        rand::random(),
    );
    let output = Output::from(
        BasicOutputBuilder::new_with_amount(1_000_000)
            .add_unlock_condition(AddressUnlockCondition::new(Ed25519Address::from(
                rand::random::<[u8; Ed25519Address::LENGTH]>(),
            )))
            .finish()
            .unwrap(),
    );

    outputs.push((output_header, output));

    outputs
}
