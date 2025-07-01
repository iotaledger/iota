// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, Result, anyhow};
use fastcrypto::hash::HashFunction;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    IotaClient,
    rpc_types::{
        IotaObjectDataOptions, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
        ObjectChange,
    },
    types::{
        Identifier,
        base_types::{IotaAddress, ObjectID, ObjectRef},
        crypto::DefaultHash,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        signature::GenericSignature,
        transaction::{Command, ObjectArg, Transaction, TransactionData},
    },
};
use move_core_types::language_storage::StructTag;
use shared_crypto::intent::{Intent, IntentMessage};

use crate::{
    sig_utils::build_multisig,
    utils::{GAS_BUDGET, THRESHOLD, WEIGHTS, compile_package, get_coin},
};

const SMART_ACC_MODULE_NAME: &str = "smart_account";

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
    let compiled_package = compile_package("../aa_move")?;

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
            GAS_BUDGET,
        )
        .await?;

    let sigs: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&alice_addr, &tx_data, Intent::iota_transaction())?
            .into(),
        keystore
            .sign_secure(&bob_addr, &tx_data, Intent::iota_transaction())?
            .into(),
    ];

    let multisig = build_multisig(keystore, &[alice_addr, bob_addr], WEIGHTS, THRESHOLD, sigs)?;

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
    let args = vec![ptb_builder.pure(multisig_addr)?];
    ptb_builder.command(Command::move_call(
        package_id,
        Identifier::new(SMART_ACC_MODULE_NAME)?,
        Identifier::new("init_multisig_smart_account")?,
        vec![],
        args,
    ));

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;
    let publisher_coin = get_coin(iota_client, publisher_addr).await?;

    println!("\nPublisher gas coin - {publisher_coin:?}");

    let tx_data = TransactionData::new_programmable(
        publisher_addr,
        vec![publisher_coin.object_ref()],
        ptb_builder.finish(),
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
    let created = smart_account_tx
        .object_changes
        .ok_or_else(|| anyhow!("No object changes found"))?;
    let find_ref = |name: &str| -> Result<ObjectRef> {
        let id = Identifier::new(name)?;
        created
            .iter()
            .find_map(|obj_change| match obj_change {
                ObjectChange::Created {
                    object_type: StructTag { name: n, .. },
                    ..
                } if n == &id => Some(obj_change.object_ref()),
                _ => None,
            })
            .ok_or_else(|| anyhow!("{name} not found"))
    };
    Ok((find_ref("SmartAccount")?, find_ref("OwnerCap")?))
}

/// Submits a deposit transaction into a SmartAccount.
/// Receives the sent coin in place.
pub async fn make_deposit_to_smart_account(
    iota_client: &IotaClient,
    keystore: &FileBasedKeystore,
    depositor_addr: IotaAddress,
    approver_addr: IotaAddress,
    multisig_addr: IotaAddress,
    package_id: ObjectID,
    smart_account_obj: ObjectRef,
) -> Result<()> {
    let depositor_coin = get_coin(iota_client, depositor_addr).await?;
    let depositor_gas_coin_for_deposit = depositor_coin.object_ref();

    println!("\n Depositor's gas coin for deposit - {depositor_gas_coin_for_deposit:?}");

    let mut ptb_builder = ProgrammableTransactionBuilder::new();

    // Deposit some coins to the smart account
    ptb_builder.transfer_object(smart_account_obj.0.into(), depositor_gas_coin_for_deposit)?;

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let depositor_coin = iota_client
        .coin_read_api()
        .select_coins(
            depositor_addr,
            None,
            1,
            vec![depositor_gas_coin_for_deposit.0],
        )
        .await?;
    let depositor_gas_coin = depositor_coin.first().unwrap();

    println!("\n Depositor's gas coin for gas payment - {depositor_gas_coin:?}");

    let tx_data = TransactionData::new_programmable(
        depositor_addr,
        vec![depositor_gas_coin.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
    );

    let signature = keystore.sign_secure(&depositor_addr, &tx_data, Intent::iota_transaction())?;

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Deposit tokens tx info: {transaction_response}");

    let mut ptb_builder = ProgrammableTransactionBuilder::new();

    let depositor_gas_coin_for_deposit = iota_client
        .read_api()
        .get_object_with_options(
            depositor_gas_coin_for_deposit.0,
            IotaObjectDataOptions::default().with_bcs(),
        )
        .await?
        .data
        .ok_or_else(|| anyhow!("Depositor's gas coin for deposit object data is missing"))?;

    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_obj.0,
            initial_shared_version: smart_account_obj.1,
            mutable: true,
        })?,
        ptb_builder.obj(ObjectArg::Receiving(
            depositor_gas_coin_for_deposit.object_ref(),
        ))?,
    ];
    ptb_builder.command(Command::move_call(
        package_id,
        Identifier::new(SMART_ACC_MODULE_NAME)?,
        Identifier::new("receive_deposit")?,
        vec![],
        arguments,
    ));

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let depositor_coin = iota_client
        .coin_read_api()
        .select_coins(
            depositor_addr,
            None,
            1,
            vec![depositor_gas_coin_for_deposit.object_id],
        )
        .await?;
    let depositor_gas_coin = depositor_coin.first().unwrap();

    let receive_tx_data = TransactionData::new_programmable_allow_sponsor(
        multisig_addr,
        vec![depositor_gas_coin.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
        depositor_addr,
    );

    let depositor_sig: GenericSignature = keystore
        .sign_secure(
            &depositor_addr,
            &receive_tx_data,
            Intent::iota_transaction(),
        )?
        .into();
    let sigs: Vec<GenericSignature> = vec![
        depositor_sig.clone(),
        keystore
            .sign_secure(&approver_addr, &receive_tx_data, Intent::iota_transaction())?
            .into(),
    ];
    let multisig = build_multisig(
        keystore,
        &[depositor_addr, approver_addr],
        WEIGHTS,
        THRESHOLD,
        sigs,
    )?;

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(receive_tx_data, vec![multisig, depositor_sig]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Receiving tokens tx info: {transaction_response}");

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
    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_obj.0,
            initial_shared_version: smart_account_obj.1,
            mutable: true,
        })?,
        ptb_builder.obj(ObjectArg::ImmOrOwnedObject(owner_cap_obj))?,
        ptb_builder.pure(withdraw_amount)?,
    ];
    let coin = ptb_builder.programmable_move_call(
        package_id,
        Identifier::new(SMART_ACC_MODULE_NAME)?,
        Identifier::new("withdraw")?,
        vec![],
        arguments,
    );

    ptb_builder.transfer_arg(coin_recipient_addr, coin);

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let alice_gas_coin = iota_client
        .coin_read_api()
        .get_all_coins(alice_addr, None, 5)
        .await?;
    let alice_gas_coin_for_sponsoring = &alice_gas_coin.data[2];

    let withdraw_tx_data = TransactionData::new_programmable_allow_sponsor(
        multisig_addr,
        vec![alice_gas_coin_for_sponsoring.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
        alice_addr,
    );

    // Hash the transaction with its intent to produce a digest
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), withdraw_tx_data.clone());
    let mut hasher = DefaultHash::default();
    hasher.update(bcs::to_bytes(&intent_msg)?);
    let digest = hasher.finalize().digest.to_vec();

    Ok((digest, withdraw_tx_data))
}

/// Calls the `delete_multisig_smart_account` function to delete a SmartAccount
/// and OwnerCap objects.
/// It returns the remained balance as coin and transfer it to the initiator.
pub async fn delete_smart_account(
    iota_client: &IotaClient,
    package_id: ObjectID,
    multisig_addr: IotaAddress,
    initiator_addr: IotaAddress,
    approver_addr: IotaAddress,
    smart_account_obj: ObjectRef,
    owner_cap_obj_id: ObjectID,
    keystore: &FileBasedKeystore,
) -> Result<IotaTransactionBlockResponse> {
    let mut ptb_builder = ProgrammableTransactionBuilder::new();

    // Get an up-to-date owner_cap object (with the last version)
    let owner_cap_obj = iota_client
        .read_api()
        .get_object_with_options(owner_cap_obj_id, IotaObjectDataOptions::new().with_bcs())
        .await?
        .data
        .ok_or(anyhow!("Owner cap object not found"))?;

    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_obj.0,
            initial_shared_version: smart_account_obj.1,
            mutable: true,
        })?,
        ptb_builder.obj(ObjectArg::ImmOrOwnedObject(owner_cap_obj.object_ref()))?,
    ];

    let coin = ptb_builder.programmable_move_call(
        package_id,
        Identifier::new(SMART_ACC_MODULE_NAME)?,
        Identifier::new("delete_multisig_smart_account")?,
        vec![],
        arguments,
    );

    ptb_builder.transfer_arg(initiator_addr, coin);

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let initiator_coin = get_coin(iota_client, initiator_addr).await?;

    println!("\n Initiator gas coin  - {initiator_addr:?}");

    let delete_sm_tx = TransactionData::new_programmable_allow_sponsor(
        multisig_addr,
        vec![initiator_coin.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
        initiator_addr,
    );

    let initiator_sig: GenericSignature = keystore
        .sign_secure(&initiator_addr, &delete_sm_tx, Intent::iota_transaction())?
        .into();

    let sigs: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&approver_addr, &delete_sm_tx, Intent::iota_transaction())?
            .into(),
        initiator_sig.clone(),
    ];

    let multisig = build_multisig(
        keystore,
        &[approver_addr, initiator_addr],
        WEIGHTS,
        THRESHOLD,
        sigs,
    )?;

    Ok(iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(delete_sm_tx, vec![multisig, initiator_sig]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?)
}
