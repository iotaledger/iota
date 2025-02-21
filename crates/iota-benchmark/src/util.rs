// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    crypto::{AccountKeyPair, IotaKeyPair, KeypairTraits},
    object::Owner,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData},
    utils::to_sender_signed_transaction,
};

use crate::{ValidatorProxy, workloads::Gas};

// This is the maximum gas we will transfer from primary coin into any gas coin
// for running the benchmark

pub type UpdatedAndNewlyMintedGasCoins = Vec<Gas>;

pub fn get_ed25519_keypair_from_keystore(
    keystore_path: PathBuf,
    requested_address: &IotaAddress,
) -> Result<AccountKeyPair> {
    let keystore = FileBasedKeystore::new(&keystore_path)?;
    match keystore.get_key(requested_address) {
        Ok(IotaKeyPair::Ed25519(kp)) => Ok(kp.copy()),
        other => Err(anyhow::anyhow!("Invalid key type: {:?}", other)),
    }
}

pub fn make_pay_tx(
    input_coins: Vec<ObjectRef>,
    sender: IotaAddress,
    addresses: Vec<IotaAddress>,
    split_amounts: Vec<u64>,
    gas: ObjectRef,
    keypair: &AccountKeyPair,
    gas_price: u64,
) -> Result<Transaction> {
    let pay = TransactionData::new_pay(
        sender,
        input_coins,
        addresses,
        split_amounts,
        gas,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER * gas_price,
        gas_price,
    )?;
    Ok(to_sender_signed_transaction(pay, keypair))
}

pub async fn publish_basics_package(
    gas: ObjectRef,
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    sender: IotaAddress,
    keypair: &AccountKeyPair,
    gas_price: u64,
) -> ObjectRef {
    let transaction = TestTransactionBuilder::new(sender, gas, gas_price)
        .publish_examples("basics")
        .build_and_sign(keypair);
    let effects = proxy.execute_transaction_block(transaction).await.unwrap();
    effects
        .created()
        .iter()
        .find(|(_, owner)| matches!(owner, Owner::Immutable))
        .map(|(reference, _)| *reference)
        .unwrap()
}
