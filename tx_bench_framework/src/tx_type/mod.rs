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
    rpc_types::{IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        IOTA_FRAMEWORK_ADDRESS,
        base_types::IotaAddress,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID, TypeTag,
    base_types::{ObjectID, ObjectRef},
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
    coins: &mut CoinRefs,
) -> Result<SubmitResult> {
    let start = std::time::Instant::now();

    let opts = IotaTransactionBlockResponseOptions::new()
        .with_effects()
        .with_object_changes();

    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            opts,
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

    refresh_coin_refs_from_changes(resp.object_changes.as_ref(), coins);

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

#[derive(Debug, Clone, Copy)]
pub struct CoinRefs {
    pub gas: ObjectRef,
    pub pay: ObjectRef,
}

fn find_updated_ref(changes: &[ObjectChange], id: ObjectID) -> Option<ObjectRef> {
    changes.iter().find_map(|ch| {
        let obj_ref = ch.object_ref();
        (obj_ref.0 == id).then_some(obj_ref)
    })
}

pub fn refresh_coin_refs_from_changes(changes: Option<&Vec<ObjectChange>>, coins: &mut CoinRefs) {
    let Some(ch) = changes else {
        return;
    };

    if let Some(new_gas) = find_updated_ref(ch, coins.gas.0) {
        coins.gas = new_gas;
    }
    if let Some(new_pay) = find_updated_ref(ch, coins.pay.0) {
        coins.pay = new_pay;
    }
}
