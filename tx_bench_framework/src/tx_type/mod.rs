// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod abstract_account_tx;
pub mod simple_tx;

use std::sync::LazyLock;

pub use abstract_account_tx::submit_aa_tx;
use anyhow::{Context, Result};
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_sdk::{
    IotaClient,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        IOTA_FRAMEWORK_ADDRESS,
        base_types::IotaAddress,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID, TypeTag,
    base_types::ObjectRef,
    iota_system_state::IOTA_SYSTEM_MODULE_NAME,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, ObjectArg},
};
use move_core_types::{ident_str, identifier::Identifier, language_storage::StructTag};
pub use simple_tx::submit_standard_tx;

pub struct SubmitResult {
    pub digest: String,
    pub gas_used: Option<String>,
    pub elapsed_ms: u128,
}

/// Cache commonly used type tags (avoid rebuilding on each submit).
static IOTA_TYPE: LazyLock<TypeTag> = LazyLock::new(|| {
    TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("iota").to_owned(),
        name: ident_str!("IOTA").to_owned(),
        type_params: vec![],
    }))
});

static COIN_IOTA_TYPE: LazyLock<TypeTag> = LazyLock::new(|| {
    TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("coin").to_owned(),
        name: ident_str!("Coin").to_owned(),
        type_params: vec![IOTA_TYPE.clone()],
    }))
});

pub fn build_split_and_transfer_pt(
    pay_coin_ref: ObjectRef,
    recipient: IotaAddress,
    split_amount: u64,
) -> Result<iota_sdk::types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();

    let split_amount_arg = b.pure(split_amount)?;
    let pay_coin_arg = b.obj(ObjectArg::ImmOrOwnedObject(pay_coin_ref))?;

    let split_res = b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("coin").to_owned(),
        ident_str!("split").to_owned(),
        vec![IOTA_TYPE.clone()],
        vec![pay_coin_arg, split_amount_arg],
    );

    let recipient_arg = b.pure(recipient)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("transfer").to_owned(),
        ident_str!("public_transfer").to_owned(),
        vec![COIN_IOTA_TYPE.clone()],
        vec![split_res, recipient_arg],
    );

    Ok(b.finish())
}

pub fn build_request_add_stake_pt(
    pay_coin_ref: ObjectRef,
    recipient: IotaAddress,
) -> Result<iota_sdk::types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();

    let pay_coin_arg = b.obj(ObjectArg::ImmOrOwnedObject(pay_coin_ref))?;
    let addr_arg = b
        .input(CallArg::Pure(bcs::to_bytes(&recipient).unwrap()))
        .unwrap();
    let state = b.input(CallArg::IOTA_SYSTEM_MUT).unwrap();
    b.programmable_move_call(
        IOTA_SYSTEM_PACKAGE_ID,
        Identifier::new(IOTA_SYSTEM_MODULE_NAME.as_str())?,
        Identifier::new("request_add_stake")?,
        vec![],
        vec![state, pay_coin_arg, addr_arg],
    );

    Ok(b.finish())
}

/// Shared execution path for both standard and AA transactions.
pub async fn execute_and_measure(
    client: &IotaClient,
    tx_data: TransactionData,
    signatures: Vec<iota_types::signature::GenericSignature>,
    wait_mode: ExecuteTransactionRequestType,
) -> Result<SubmitResult> {
    let start = std::time::Instant::now();

    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(wait_mode),
        )
        .await
        .context("execute_transaction_block failed")?;

    if !resp.errors.is_empty() {
        eprintln!(
            "Transaction failed: {:?}, digest={}",
            resp.errors, resp.digest
        );
    }

    let elapsed_ms = start.elapsed().as_millis();
    Ok(SubmitResult {
        digest: resp.digest.to_string(),
        gas_used: resp
            .effects
            .as_ref()
            .map(|e| format!("{:?}", e.gas_cost_summary())),
        elapsed_ms,
    })
}
