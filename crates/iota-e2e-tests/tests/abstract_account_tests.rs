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
    transaction::{
        Argument, CallArg, ObjectArg, ProgrammableTransaction,
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction, TransactionData,
    },
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;
use test_cluster::{TestCluster, TestClusterBuilder};

const ABSTRACTACCOUNT_PACKAGE_PATH: &str = "tests/abstract_account/abstract_account";
const ABSTRACTACCOUNT_MODULE_NAME: &str = "abstract_account";
const ABSTRACTACCOUNT_CREATE_MODULE_NAME: &str = "basic_keyed_aa";
const ABSTRACTACCOUNT_AUTHENTICATE_MODULE_NAME: &str = "basic_keyed_aa";
const ABSTRACTACCOUNT_AUTHENTICATE_FN_NAME_ED25519: &str = "authenticate_ed25519";
const ABSTRACTACCOUNT_AUTHENTICATE_FN_NAME_FREE_ACCESS: &str = "authenticate_free_access";

#[sim_test]
async fn test_abstract_account_creation_and_issue_tx() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(ABSTRACTACCOUNT_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let abstractaccount_ref = test_env.abstractaccount_ref.unwrap();

    // Retrieve the keystore
    let keystore = test_env.test_cluster.wallet.config().keystore();
    let abstractaccount_sender = abstractaccount_ref.0.into();

    // Request faucet coins for the AbstractAccount
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let abstractaccount_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), abstractaccount_sender)
        .await;

    // Create a simple transaction from the IOTA account
    let pt = test_env.abstract_account_simple_tx()?;
    let tx_data = test_env
        .craft_tx_from_pt(
            pt,
            abstractaccount_gas,
            abstractaccount_sender,
            None, // No sponsor
        )
        .await?;
    let tx_digest = tx_data.digest().into_inner();

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
    // Sign the tx data with the owner key
    let hex_encoded_signature: String =
        Hex::encode(keystore.sign_hashed(&test_env.owner.unwrap(), &tx_digest)?)
            .chars()
            .skip(2) // flag prefix length
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
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
    test_env
        .execute_and_check_tx_correctness(abstractaccount_simple_tx)
        .await
}

#[sim_test]
async fn test_abstract_account_issues_sponsored_tx() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(ABSTRACTACCOUNT_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let abstractaccount_ref = test_env.abstractaccount_ref.unwrap();

    // Retrieve the keystore and derive the address of the first account
    let keystore = test_env.test_cluster.wallet.config().keystore();
    let sponsor = keystore.addresses().first().cloned().unwrap();

    // Request faucet coins for the Sponsor
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), sponsor)
        .await;

    // Create a simple transaction from the IOTA account
    let pt = test_env.abstract_account_simple_tx()?;
    let abstractaccount_sender = abstractaccount_ref.0.into();
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, abstractaccount_sender, Some(sponsor))
        .await?;

    // Sponsor signature
    let sponsor_signature = GenericSignature::Signature(keystore.sign_secure(
        &sponsor,
        &tx_data,
        Intent::iota_transaction(),
    )?);

    // Create the MoveAuthenticator for the free access authenticator:
    // public fun authenticate_free_access(
    //    self: &AbstractAccount,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: abstractaccount_ref.0,
        initial_shared_version: abstractaccount_ref.1,
        mutable: false,
    });
    let abstractaccount_signature = GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_for_testing(vec![self_call_arg.clone()], vec![], self_call_arg),
    );

    // Create the TX envelope and execute it
    let abstractaccount_sponsored_tx = Transaction::from_generic_sig_data(
        tx_data,
        vec![abstractaccount_signature, sponsor_signature],
    );
    test_env
        .execute_and_check_tx_correctness(abstractaccount_sponsored_tx)
        .await
}

struct TestEnvironment {
    test_cluster: TestCluster,
    owner: Option<IotaAddress>,
    authenticate_fn_name: Option<String>,
    abstractaccount_package_id: Option<ObjectID>,
    abstractaccount_ref: Option<ObjectRef>,
}

impl TestEnvironment {
    async fn new() -> Self {
        let test_cluster = TestClusterBuilder::new().build().await;

        Self {
            test_cluster,
            owner: None,
            authenticate_fn_name: None,
            abstractaccount_package_id: None,
            abstractaccount_ref: None,
        }
    }

    async fn setup_abstract_account(
        &mut self,
        authenticate_fn_name: &str,
    ) -> Result<(), anyhow::Error> {
        // Store the authenticate function name
        self.authenticate_fn_name = Some(authenticate_fn_name.to_string());

        // Retrieve the keystore and derive the address of the first account
        self.owner = Some(
            self.test_cluster
                .wallet
                .config()
                .keystore()
                .addresses()
                .first()
                .cloned()
                .unwrap(),
        );

        // Publish the Move Account Abstraction package
        self.abstractaccount_package_id = Some(self.publish_account_abstraction_package().await);

        // Create an AbstractAccount
        self.abstractaccount_ref = Some(self.create_abstract_account().await?);

        Ok(())
    }

    async fn publish_account_abstraction_package(&mut self) -> ObjectID {
        let path = [env!("CARGO_MANIFEST_DIR"), ABSTRACTACCOUNT_PACKAGE_PATH]
            .iter()
            .collect();
        publish_package(self.test_cluster.wallet(), path).await.0
    }

    async fn create_abstract_account(&self) -> anyhow::Result<ObjectRef> {
        let (Some(owner), Some(authenticate_fn_name), Some(abstractaccount_package_id)) = (
            self.owner,
            &self.authenticate_fn_name,
            self.abstractaccount_package_id,
        ) else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let aa_owner_pk = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .get_key(&owner)?
            .public();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            // create auth info
            let arguments = vec![
                builder.pure(abstractaccount_package_id)?,
                builder.pure(ABSTRACTACCOUNT_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
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
                    ident_str!(ABSTRACTACCOUNT_CREATE_MODULE_NAME).to_owned(),
                    ident_str!("create").to_owned(),
                    vec![],
                    arguments,
                );
            }
            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(pt)
            .build();

        let transaction = self.test_cluster.wallet.sign_transaction(&tx_data);
        let (effects, _) = self
            .test_cluster
            .execute_transaction_return_raw_effects(transaction)
            .await?;

        // Extract the only created shared object which is the abstract account
        Ok(effects
            .all_changed_objects()
            .iter()
            .find_map(|change| match change {
                (_, Owner::Shared { .. }, WriteKind::Create) => Some(change.0),
                _ => None,
            })
            .expect("Expected a shared object in the transaction response"))
    }

    fn abstract_account_simple_tx(&self) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(abstractaccount_ref), Some(abstractaccount_package_id)) =
            (self.abstractaccount_ref, self.abstractaccount_package_id)
        else {
            anyhow::bail!("Abstract account not created yet");
        };
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
        Ok(builder.finish())
    }

    // Utilities

    async fn craft_tx_from_pt(
        &self,
        pt: ProgrammableTransaction,
        gas_coin: ObjectRef,
        sender: IotaAddress,
        sponsor: Option<IotaAddress>,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.test_cluster.get_reference_gas_price().await;

        // Create the transaction data that will be sent to the network
        Ok(TransactionData::new_programmable_allow_sponsor(
            sender,
            vec![gas_coin],
            pt,
            gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            gas_price,
            sponsor.unwrap_or(sender),
        ))
    }

    async fn execute_and_check_tx_correctness(&self, tx: Transaction) -> anyhow::Result<()> {
        let transaction_response = self.test_cluster.execute_transaction(tx).await;

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
}
