// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::IotaCallArg;
use iota_types::{
    base_types::{IotaAddress, ObjectDigest, ObjectID, SequenceNumber},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData},
};
use move_core_types::annotated_value::MoveTypeLayout;

use crate::{operations::Operations, types::ConstructionMetadata};

#[tokio::test]
async fn test_operation_data_parsing() -> Result<(), anyhow::Error> {
    let gas = (
        ObjectID::random(),
        SequenceNumber::new(),
        ObjectDigest::random(),
    );

    let sender = IotaAddress::random_for_testing_only();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay_iota(vec![IotaAddress::random_for_testing_only()], vec![10000])
            .unwrap();
        builder.finish()
    };
    let gas_price = 10;
    let data = TransactionData::new_programmable(
        sender,
        vec![gas],
        pt,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER * gas_price,
        gas_price,
    );

    let ops: Operations = Operations::from_transaction_data(data.clone(), None)?;
    let metadata = ConstructionMetadata {
        sender,
        coins: vec![gas],
        objects: vec![],
        total_coin_value: 0,
        gas_price,
        budget: TEST_ONLY_GAS_UNIT_FOR_TRANSFER * gas_price,
    };
    let parsed_data = ops.into_internal()?.try_into_data(metadata)?;
    assert_eq!(data, parsed_data);

    Ok(())
}
#[tokio::test]
async fn test_iota_json() {
    let arg1 = CallArg::Pure(bcs::to_bytes(&1000000u64).unwrap());
    let arg2 = CallArg::Pure(bcs::to_bytes(&30215u64).unwrap());
    let json1 = IotaCallArg::try_from(arg1, Some(&MoveTypeLayout::U64)).unwrap();
    let json2 = IotaCallArg::try_from(arg2, Some(&MoveTypeLayout::U64)).unwrap();
    println!("{:?}, {:?}", json1, json2);
}
