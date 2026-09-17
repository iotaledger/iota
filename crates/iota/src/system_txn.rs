// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared plumbing for the CLI commands that call entry points on the system
//! object at `0x5`, such as `iota validator` and `iota attestor`.

use anyhow::{Result, bail};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_sdk::{IotaClient, wallet_context::WalletContext};
use iota_sdk_types::{Address, Identifier, ObjectId, ObjectReference, Transaction};
use iota_types::{
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, TransactionAPI, TransactionEnvelope},
};

use crate::signing::sign_transaction;

/// Call `function` on the system object as the active address, and wait for
/// the transaction to execute locally.
pub(crate) async fn call_0x5(
    context: &mut WalletContext,
    function: &'static str,
    call_args: Vec<CallArg>,
    gas_budget: u64,
) -> Result<IotaTransactionBlockResponse> {
    let sender = context.active_address()?;
    let tx_data =
        construct_unsigned_0x5_txn(context, sender, function, call_args, gas_budget).await?;
    execute_0x5_txn(context, tx_data).await
}

/// A move call on the system object, taking the mutable system state as its
/// first argument and a gas coin that covers `gas_budget`.
async fn construct_unsigned_0x5_txn(
    context: &mut WalletContext,
    sender: Address,
    function: &'static str,
    call_args: Vec<CallArg>,
    gas_budget: u64,
) -> Result<Transaction> {
    let iota_client = context.get_client().await?;
    let mut args = vec![CallArg::IOTA_SYSTEM_MUTABLE];
    args.extend(call_args);
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await?;

    let gas_obj_ref = get_gas_obj_ref(sender, &iota_client, gas_budget).await?;
    Transaction::new_move_call(
        sender,
        ObjectId::SYSTEM,
        Identifier::IOTA_SYSTEM_MODULE,
        Identifier::from_static(function),
        vec![],
        gas_obj_ref,
        args,
        gas_budget,
        rgp,
    )
}

/// Sign `tx_data` with the sender's key and execute it, waiting for the local
/// execution to finish.
pub(crate) async fn execute_0x5_txn(
    context: &mut WalletContext,
    tx_data: Transaction,
) -> Result<IotaTransactionBlockResponse> {
    let iota_client = context.get_client().await?;
    let signature = sign_transaction(context, &tx_data, &tx_data.sender(), None).await?;
    let transaction = TransactionEnvelope::from_user_sig_data(tx_data, vec![signature]);

    iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            transaction,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

/// A single IOTA coin of `iota_address` holding at least
/// `minimal_gas_balance`, to pay for one transaction.
pub(crate) async fn get_gas_obj_ref(
    iota_address: Address,
    iota_client: &IotaClient,
    minimal_gas_balance: u64,
) -> Result<ObjectReference> {
    let coins = iota_client
        .coin_read_api()
        .get_coins(iota_address, Some("0x2::iota::IOTA".into()), None, None)
        .await?
        .data;
    let Some(gas_obj) = coins.iter().find(|c| c.balance >= minimal_gas_balance) else {
        bail!(
            "no single IOTA coin of {iota_address} covers this transaction; \
             at least {minimal_gas_balance} nanos are needed"
        );
    };
    Ok(gas_obj.object_ref())
}
