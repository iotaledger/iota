// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, Result, anyhow};
use fastcrypto::hash::HashFunction;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    IotaClient,
    rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        Identifier,
        base_types::{IotaAddress, ObjectID, ObjectRef},
        crypto::DefaultHash,
        multisig::{MultiSig, MultiSigPublicKey},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        signature::GenericSignature,
        transaction::{Command, GasData, ObjectArg, Transaction, TransactionData, TransactionKind},
    },
};
use move_core_types::language_storage::StructTag;
use shared_crypto::intent::{Intent, IntentMessage};

use crate::utils::{GAS_BUDGET, compile_package, get_coin};

const AA_MODULE_NAME: &str = "account_abstraction";

/// Publishes the Move Account Abstraction (AA) package on-chain using a
/// multisig account.
///
/// # Arguments
/// - `iota_client`: Initialized IOTA client instance.
/// - `alice_addr`, `bob_addr`: Signers for the multisig.
/// - `multisig_addr`: Address used to pay for package deployment.
/// - `keystore`: Source of secret keys for signing.
///
/// # Returns
/// The transaction response that includes the published package.
pub async fn publish_account_abstraction_package(
    iota_client: &IotaClient,
    alice_addr: IotaAddress,
    bob_addr: IotaAddress,
    multisig_addr: IotaAddress,
    keystore: &FileBasedKeystore,
) -> Result<IotaTransactionBlockResponse> {
    // Build the Move package from source
    let compiled_package = compile_package("../../aa_move")?;

    // Get multisig_addr coin for payment
    let multisig_addr_gas_coin = get_coin(iota_client, multisig_addr).await?;

    // Prepare publish transaction
    let tx_data = iota_client
        .transaction_builder()
        .publish(
            multisig_addr,
            compiled_package.get_package_bytes(false),
            compiled_package.get_dependency_storage_package_ids(),
            multisig_addr_gas_coin.coin_object_id,
            100000000,
        )
        .await?;

    // Create individual signatures
    let alice_sig: GenericSignature = keystore
        .sign_secure(&alice_addr, &tx_data, Intent::iota_transaction())?
        .into();
    let bob_sig: GenericSignature = keystore
        .sign_secure(&bob_addr, &tx_data, Intent::iota_transaction())?
        .into();

    // Construct multisig public key and aggregate signature
    let multisig_pub_key = MultiSigPublicKey::new(
        vec![
            keystore.get_key(&alice_addr)?.public(),
            keystore.get_key(&bob_addr)?.public(),
        ],
        vec![1, 2],
        2,
    )?;

    let multisig: GenericSignature =
        MultiSig::combine(vec![alice_sig, bob_sig], multisig_pub_key)?.into();

    // Deployment of the Account Abstraction (AA) Package via Multisig:
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, vec![multisig]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;
    Ok(transaction_response)
}

/// Calls the `init_multisig_aa` function to create a SmartAccount object.
///
/// # Returns
/// A response that includes the created objects, including SmartAccount and
/// OwnerCap.
pub async fn init_smart_account(
    iota_client: &IotaClient,
    package_id: ObjectID,
    publisher_addr: IotaAddress,
    multisig_addr: IotaAddress,
    keystore: &FileBasedKeystore,
) -> Result<IotaTransactionBlockResponse> {
    let mut ptb_builder = ProgrammableTransactionBuilder::new();
    let module = Identifier::new(AA_MODULE_NAME)?;
    let function = Identifier::new("init_multisig_aa")?;

    let arguments = vec![ptb_builder.pure(multisig_addr)?];
    ptb_builder.command(Command::move_call(
        package_id,
        module,
        function,
        vec![],
        arguments,
    ));

    // build the transaction block by calling finish on the ptb
    let ptb = ptb_builder.finish();

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let publisher_coin = get_coin(iota_client, publisher_addr).await?;

    println!("\n *** Publisher gas coin ***");
    println!("{publisher_coin:?}");

    let tx_data = TransactionData::new_programmable(
        publisher_addr,
        vec![publisher_coin.object_ref()],
        ptb,
        GAS_BUDGET,
        gas_price,
    );

    let sig = keystore.sign_secure(&publisher_addr, &tx_data, Intent::iota_transaction())?;

    Ok(iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![sig]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?)
}

/// Extracts the object references (ObjectRef) for `SmartAccount` and `OwnerCap`
/// from transaction response.
pub fn smart_account_data(
    smart_account_tx: IotaTransactionBlockResponse,
) -> Result<(ObjectRef, ObjectRef)> {
    let aa_name = Identifier::new("SmartAccount")?;
    let owner_cap_name = Identifier::new("OwnerCap")?;
    let created = smart_account_tx
        .object_changes
        .as_ref()
        .ok_or_else(|| anyhow!("No object changes found"))?;

    let find_object = |name: &Identifier| -> Option<ObjectRef> {
        created.iter().find_map(|change| match change {
            ObjectChange::Created {
                object_type: StructTag { name: n, .. },
                ..
            } if n == name => Some(change.object_ref()),
            _ => None,
        })
    };

    let smart_account = find_object(&aa_name).ok_or_else(|| anyhow!("SmartAccount not found"))?;
    let owner_cap = find_object(&owner_cap_name).ok_or_else(|| anyhow!("OwnerCap not found"))?;

    Ok((smart_account, owner_cap))
}

/// Submits a deposit transaction into a SmartAccount using Alice's gas coin.
pub async fn make_deposit_to_smart_account(
    iota_client: &IotaClient,
    keystore: &FileBasedKeystore,
    alice_addr: IotaAddress,
    package_id: ObjectID,
    smart_account_obj: ObjectRef,
) -> Result<()> {
    let alice_coin = get_coin(iota_client, alice_addr).await?;
    let alice_gas_coin_for_deposit = alice_coin.object_ref();

    println!("\n *** Alice gas coin for deposit***");
    println!("{alice_gas_coin_for_deposit:?}");

    let mut ptb_builder = ProgrammableTransactionBuilder::new();
    let module = Identifier::new(AA_MODULE_NAME)?;
    let function = Identifier::new("deposit")?;

    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_obj.0,
            initial_shared_version: smart_account_obj.1,
            mutable: true,
        })?,
        ptb_builder.obj(ObjectArg::ImmOrOwnedObject(alice_gas_coin_for_deposit))?,
    ];
    ptb_builder.command(Command::move_call(
        package_id,
        module,
        function,
        vec![],
        arguments,
    ));

    let ptb = ptb_builder.finish();
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let alice_coin = iota_client
        .coin_read_api()
        .select_coins(alice_addr, None, 1, vec![alice_gas_coin_for_deposit.0])
        .await?;
    let alice_gas_coin = alice_coin.first().unwrap();

    println!("\n *** Alice gas coin for gas payment ***");
    println!("{alice_gas_coin:?}");

    let tx_data = TransactionData::new_programmable(
        alice_addr,
        vec![alice_gas_coin.object_ref()],
        ptb,
        GAS_BUDGET,
        gas_price,
    );

    let signature = keystore.sign_secure(&alice_addr, &tx_data, Intent::iota_transaction())?;

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Deposit tx info: ");
    println!("{}", transaction_response);

    Ok(())
}

/// Builds a withdrawal transaction from a SmartAccount,
/// returning the transaction digest and prepared TransactionData.
///
/// This will later be used to propose the transaction and collect signatures.
pub async fn prepare_withdraw_tx_data(
    iota_client: &IotaClient,
    alice_addr: IotaAddress,
    multisig_addr: IotaAddress,
    package_id: ObjectID,
    smart_account_obj: ObjectRef,
    owner_cap_obj: ObjectRef,
    coin_recipient_addr: IotaAddress,
    withdraw_amount: u64,
) -> Result<(Vec<u8>, TransactionData)> {
    let mut ptb_builder = ProgrammableTransactionBuilder::new();
    let module = Identifier::new(AA_MODULE_NAME).map_err(|e| anyhow!(e))?;
    let function = Identifier::new("withdraw").map_err(|e| anyhow!(e))?;
    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_obj.0,
            initial_shared_version: smart_account_obj.1,
            mutable: true,
        })?,
        ptb_builder.obj(ObjectArg::ImmOrOwnedObject(owner_cap_obj))?,
        ptb_builder.pure(withdraw_amount)?,
        ptb_builder.pure(coin_recipient_addr)?,
    ];
    ptb_builder.command(Command::move_call(
        package_id,
        module,
        function,
        vec![],
        arguments,
    ));

    let ptb = ptb_builder.finish();
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let alice_gas_coin = iota_client
        .coin_read_api()
        .get_all_coins(alice_addr, None, 5)
        .await?;
    let alice_gas_coin_for_sponsoring = &alice_gas_coin.data[2];

    let gas_data = GasData {
        payment: vec![alice_gas_coin_for_sponsoring.object_ref()],
        owner: alice_addr,
        price: gas_price,
        budget: GAS_BUDGET,
    };
    let withdraw_tx_data = TransactionData::new_with_gas_data(
        TransactionKind::programmable(ptb),
        multisig_addr,
        gas_data.clone(),
    );

    // Hash the transaction with its intent to produce a digest
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), withdraw_tx_data.clone());
    let mut hasher = DefaultHash::default();
    hasher.update(bcs::to_bytes(&intent_msg)?);
    let digest = hasher.finalize().digest.to_vec();

    Ok((digest, withdraw_tx_data))
}
