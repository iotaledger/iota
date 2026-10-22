// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::transaction::{CallArg, DEFAULT_VALIDATOR_GAS_PRICE, TransactionEnvelope};

use crate::{mock_account::Account, tx_generator::TxGenerator};

/// Mints `objects_per_account` NFTs to each account at setup, as fixtures
/// for the mutate-in-place and burn workloads.
pub struct OwnedObjectCreateTxGenerator {
    move_package: ObjectId,
    objects_per_account: u64,
    object_size: u16,
}

impl OwnedObjectCreateTxGenerator {
    pub fn new(move_package: ObjectId, objects_per_account: u64, object_size: u16) -> Self {
        Self {
            move_package,
            objects_per_account,
            object_size,
        }
    }
}

impl TxGenerator for OwnedObjectCreateTxGenerator {
    fn generate_tx(&self, account: Account) -> TransactionEnvelope {
        let recipients = vec![account.sender; self.objects_per_account as usize];
        let contents = vec![7u8; (self.object_size.max(32) - 32) as usize];
        TestTransactionBuilder::new(
            account.sender,
            account.gas_objects[0],
            DEFAULT_VALIDATOR_GAS_PRICE,
        )
        .move_call(
            self.move_package,
            "benchmark",
            "batch_mint",
            vec![CallArg::pure(&recipients), CallArg::pure(&contents)],
        )
        .build_and_sign(account.private_key.as_ref())
    }

    fn name(&self) -> &'static str {
        "Owned Object Creation TransactionEnvelope Generator"
    }
}
