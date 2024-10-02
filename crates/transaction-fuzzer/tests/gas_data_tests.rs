// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::dbg_addr,
    crypto::KeypairTraits,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TransactionData, TransactionKind},
    utils::to_sender_signed_transaction,
};
use proptest::{arbitrary::*, test_runner::TestCaseError};
use tracing::debug;
use transaction_fuzzer::{GasDataGenConfig, GasDataWithObjects, executor::Executor, run_proptest};

/// Send transfer iota txn with provided random gas data and gas objects to an
/// authority.
fn test_with_random_gas_data(
    gas_data_test: GasDataWithObjects,
    executor: &mut Executor,
) -> Result<(), TestCaseError> {
    let gas_data = gas_data_test.gas_data;
    let objects = gas_data_test.objects;
    let sender = gas_data_test.sender_key.public().into();

    // Insert the random gas objects into genesis.
    executor.add_objects(&objects);
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let recipient = dbg_addr(2);
        builder.transfer_iota(recipient, None);
        builder.finish()
    };
    let kind = TransactionKind::ProgrammableTransaction(pt);
    let tx_data = TransactionData::new_with_gas_data(kind, sender, gas_data);
    let tx = to_sender_signed_transaction(tx_data, &gas_data_test.sender_key);

    let result = executor.execute_transaction(tx);
    debug!("result: {:?}", result);
    Ok(())
}

#[test]
#[cfg_attr(msim, ignore)]
fn test_gas_data_owned_or_immut() {
    let strategy = any_with::<GasDataWithObjects>(GasDataGenConfig::owned_by_sender_or_immut());
    run_proptest(1000, strategy, |gas_data_test, mut executor| {
        test_with_random_gas_data(gas_data_test, &mut executor)
    });
}

#[test]
#[cfg_attr(msim, ignore)]
fn test_gas_data_any_owner() {
    let strategy = any_with::<GasDataWithObjects>(GasDataGenConfig::any_owner());
    run_proptest(1000, strategy, |gas_data_test, mut executor| {
        test_with_random_gas_data(gas_data_test, &mut executor)
    });
}
