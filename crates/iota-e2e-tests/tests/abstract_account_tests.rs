// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Abstract Account tests
//!
//! The tests in this module are meant to test the creation of an abstracted
//! account and sending of a TX through it using a Move-based Ed25519
//! authentication.

use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, TypeTag,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    move_authenticator::MoveAuthenticator,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    storage::WriteKind,
    transaction::{Argument, CallArg, ObjectArg, Transaction, TransactionData},
};
use move_core_types::ident_str;
use test_cluster::{TestCluster, TestClusterBuilder};

const ABSTRACTACCOUNT_PACKAGE_PATH: &str = "tests/abstract_account/abstract_account";
const ABSTRACTACCOUNT_MODULE_NAME: &str = "abstract_account";

#[sim_test]
async fn test_abstract_account_creation_and_tx_issuing() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test cluster
    let mut test_cluster = TestClusterBuilder::new().build().await;

    // Publish the Move Account Abstraction package
    let abstractaccount_package_id = publish_account_abstraction_package(&mut test_cluster).await;

    // Retrieve the keystore and derive the address of the first account
    let keystore = test_cluster.wallet.config().keystore();
    let sender = keystore.addresses().first().cloned().unwrap();

    // Create an AbstractAccount
    let abstractaccount_ref =
        create_abstract_account(&test_cluster, sender, abstractaccount_package_id).await?;

    // Request faucet coins for the AbstractAccount
    let rgp = test_cluster.get_reference_gas_price().await;
    let abstractaccount_gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), abstractaccount_ref.0.into())
        .await;

    // Create a simple transaction from the IOTA account
    let tx_data = abstract_account_simple_tx(
        &test_cluster,
        abstractaccount_package_id,
        abstractaccount_ref,
        abstractaccount_gas,
    )
    .await?;
    let tx_digest = tx_data.digest().into_inner();

    // Sign the tx data with the sender key
    let signature = keystore.sign_hashed(&sender, &tx_digest)?;
    let hex_encoded_signature: String = Hex::encode(signature)
        .chars()
        .skip(2) // flag prefix length
        .take(Ed25519Signature::LENGTH * 2)
        .collect();

    // Create the MoveAuthenticator for the Ed25519 signature authenticator:
    // public fun authenticate_ed25519(
    //    self: &AbstractAccount,
    //    signature: vector<u8>,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: abstractaccount_ref.0,
        initial_shared_version: abstractaccount_ref.1,
        mutable: false,
    });
    let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
    let signatures = vec![GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_for_testing(
            vec![self_call_arg.clone(), signature_call_arg],
            vec![],
            self_call_arg,
        ),
    )];

    // Create the TX envelope and execute it
    let abstractaccount_simple_tx = Transaction::from_generic_sig_data(tx_data, signatures);
    let transaction_response = test_cluster
        .execute_transaction(abstractaccount_simple_tx)
        .await;

    // Check correctness
    let IotaTransactionBlockResponse {
        confirmed_local_execution,
        errors,
        ..
    } = transaction_response;

    // The transaction must be successful
    assert!(confirmed_local_execution.unwrap());
    assert!(errors.is_empty());
    Ok(())
}

pub async fn publish_account_abstraction_package(test_cluster: &mut TestCluster) -> ObjectID {
    let path = [env!("CARGO_MANIFEST_DIR"), ABSTRACTACCOUNT_PACKAGE_PATH]
        .iter()
        .collect();
    publish_package(test_cluster.wallet(), path).await.0
}

pub async fn create_abstract_account(
    test_cluster: &TestCluster,
    sender: IotaAddress,
    abstractaccount_package_id: ObjectID,
) -> anyhow::Result<ObjectRef> {
    let aa_owner_pk = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&sender)?
        .public();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // create auth info
        let arguments = vec![
            builder.pure(abstractaccount_package_id)?,
            builder.pure(ABSTRACTACCOUNT_MODULE_NAME)?,
            builder.pure("authenticate_ed25519")?,
        ];
        if let Argument::Result(authenticator_info_v1) = builder.programmable_move_call(
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("account").to_owned(),
            ident_str!("create_auth_info_v1").to_owned(),
            vec![],
            arguments,
        ) {
            // Create the abstract account.
            let arguments = vec![
                builder.pure(aa_owner_pk.as_ref())?,
                Argument::Result(authenticator_info_v1),
            ];
            builder.programmable_move_call(
                abstractaccount_package_id,
                ident_str!(ABSTRACTACCOUNT_MODULE_NAME).to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                arguments,
            );
        }
        builder.finish()
    };

    let tx_data = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .programmable(pt)
        .build();

    let transaction = test_cluster.wallet.sign_transaction(&tx_data);
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(transaction)
        .await?;

    // Extract the only created shared object which is the abstract account
    Ok(effects
        .all_changed_objects()
        .iter()
        .find_map(|change| match change {
            (_, Owner::Shared { .. }, WriteKind::Create) => Some(change.0.clone()),
            _ => None,
        })
        .expect("Expected a shared object in the transaction response"))
}

pub async fn abstract_account_simple_tx(
    test_cluster: &TestCluster,
    abstractaccount_package_id: ObjectID,
    abstractaccount_ref: ObjectRef,
    abstractaccount_gas: ObjectRef,
) -> anyhow::Result<TransactionData> {
    let sender = abstractaccount_ref.0.into();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Random IOTA account command.
        let arguments = vec![
            builder.obj(ObjectArg::SharedObject {
                id: abstractaccount_ref.0,
                initial_shared_version: abstractaccount_ref.1,
                mutable: true,
            })?,
            builder.pure(1_u8)?,
            builder.pure(2_u8)?,
        ];
        builder.programmable_move_call(
            abstractaccount_package_id,
            ident_str!(ABSTRACTACCOUNT_MODULE_NAME).to_owned(),
            ident_str!("add_field").to_owned(),
            vec![TypeTag::U8, TypeTag::U8],
            arguments,
        );
        builder.finish()
    };

    // Create the transaction data that will be sent to the network
    Ok(test_cluster
        .test_transaction_builder_with_gas_object(sender, abstractaccount_gas)
        .await
        .programmable(pt)
        .build())
}
