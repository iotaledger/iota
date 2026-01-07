// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod abstract_account_tx;
pub mod simple_tx;

pub use abstract_account_tx::submit_aa_tx;
use anyhow::Result;
use iota_sdk::types::{IOTA_FRAMEWORK_ADDRESS, base_types::IotaAddress};
use iota_types::{
    TypeTag, base_types::ObjectRef,
    programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::ObjectArg,
};
use move_core_types::{ident_str, language_storage::StructTag};
pub use simple_tx::submit_standard_tx;

fn build_split_and_transfer_pt(
    pay_coin_ref: ObjectRef,
    recipient: IotaAddress,
    split_amount: u64,
) -> Result<iota_sdk::types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();

    let iota_type = TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("iota").to_owned(),
        name: ident_str!("IOTA").to_owned(),
        type_params: vec![],
    }));

    let coin_iota_type = TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("coin").to_owned(),
        name: ident_str!("Coin").to_owned(),
        type_params: vec![iota_type.clone()],
    }));

    let split_amount_arg = b.pure(split_amount)?;
    let pay_coin_arg = b.obj(ObjectArg::ImmOrOwnedObject(pay_coin_ref))?;
    let split_res = b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("coin").to_owned(),
        ident_str!("split").to_owned(),
        vec![iota_type],
        vec![pay_coin_arg, split_amount_arg],
    );

    let recipient_arg = b.pure(recipient)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("transfer").to_owned(),
        ident_str!("public_transfer").to_owned(),
        vec![coin_iota_type],
        vec![split_res, recipient_arg],
    );

    Ok(b.finish())
}
