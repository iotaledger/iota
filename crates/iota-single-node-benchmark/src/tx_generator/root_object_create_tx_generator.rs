// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::ObjectID,
    transaction::{CallArg, Transaction, DEFAULT_VALIDATOR_GAS_PRICE},
};

use crate::{mock_account::Account, tx_generator::TxGenerator};

pub struct RootObjectCreateTxGenerator {
    move_package: ObjectID,
    child_per_root: u64,
}

impl RootObjectCreateTxGenerator {
    pub fn new(move_package: ObjectID, child_per_root: u64) -> Self {
        Self {
            move_package,
            child_per_root,
        }
    }
}

impl TxGenerator for RootObjectCreateTxGenerator {
    fn generate_tx(&self, account: Account) -> Transaction {
        TestTransactionBuilder::new(
            account.sender,
            account.gas_objects[0],
            DEFAULT_VALIDATOR_GAS_PRICE,
        )
        .move_call(
            self.move_package,
            "benchmark",
            "generate_dynamic_fields",
            vec![CallArg::Pure(bcs::to_bytes(&self.child_per_root).unwrap())],
        )
        .build_and_sign(account.keypair.as_ref())
    }

    fn name(&self) -> &'static str {
        "Root Object Creation Transaction Generator"
    }
}
