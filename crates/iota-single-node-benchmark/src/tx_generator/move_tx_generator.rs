// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, ObjectArg, Transaction, DEFAULT_VALIDATOR_GAS_PRICE},
};
use move_core_types::identifier::Identifier;

use crate::{mock_account::Account, tx_generator::TxGenerator};

pub struct MoveTxGenerator {
    move_package: ObjectID,
    num_transfers: u64,
    use_native_transfer: bool,
    computation: u8,
    root_objects: HashMap<IotaAddress, ObjectRef>,
}

impl MoveTxGenerator {
    pub fn new(
        move_package: ObjectID,
        num_transfers: u64,
        use_native_transfer: bool,
        computation: u8,
        root_objects: HashMap<IotaAddress, ObjectRef>,
    ) -> Self {
        Self {
            move_package,
            num_transfers,
            use_native_transfer,
            computation,
            root_objects,
        }
    }
}

impl TxGenerator for MoveTxGenerator {
    fn generate_tx(&self, account: Account) -> Transaction {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            // Step 1: transfer `num_transfers` objects.
            // First object in the gas_objects is the gas object and we are not transferring
            // it.
            for i in 1..=self.num_transfers {
                let object = account.gas_objects[i as usize];
                if self.use_native_transfer {
                    builder.transfer_object(account.sender, object).unwrap();
                } else {
                    builder
                        .move_call(
                            self.move_package,
                            Identifier::new("benchmark").unwrap(),
                            Identifier::new("transfer_coin").unwrap(),
                            vec![],
                            vec![CallArg::Object(ObjectArg::ImmOrOwnedObject(object))],
                        )
                        .unwrap();
                }
            }

            if !self.root_objects.is_empty() {
                // Step 2: Read all dynamic fields from the root object.
                let root_object = self.root_objects.get(&account.sender).unwrap();
                let root_object_arg = builder
                    .obj(ObjectArg::ImmOrOwnedObject(*root_object))
                    .unwrap();
                builder.programmable_move_call(
                    self.move_package,
                    Identifier::new("benchmark").unwrap(),
                    Identifier::new("read_dynamic_fields").unwrap(),
                    vec![],
                    vec![root_object_arg],
                );
            }

            if self.computation > 0 {
                // Step 3: Run some computation.
                let computation_arg = builder.pure(self.computation as u64 * 100).unwrap();
                builder.programmable_move_call(
                    self.move_package,
                    Identifier::new("benchmark").unwrap(),
                    Identifier::new("run_computation").unwrap(),
                    vec![],
                    vec![computation_arg],
                );
            }
            builder.finish()
        };
        TestTransactionBuilder::new(
            account.sender,
            account.gas_objects[0],
            DEFAULT_VALIDATOR_GAS_PRICE,
        )
        .programmable(pt)
        .build_and_sign(account.keypair.as_ref())
    }

    fn name(&self) -> &'static str {
        "Programmable Move Transaction Generator"
    }
}
