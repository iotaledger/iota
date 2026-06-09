// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, anyhow, bail, ensure};
use iota_sdk_types::{Command, Identifier, ObjectId};
use iota_types::{
    base_types::{IotaAddress, ObjectType},
    governance::{ADD_STAKE_MUL_COIN_FUN_NAME, WITHDRAW_STAKE_FUN_NAME},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    timelock::timelocked_staking::{
        ADD_TIMELOCKED_STAKE_FUN_NAME, WITHDRAW_TIMELOCKED_STAKE_FUN_NAME,
    },
    transaction::{CallArg, TransactionData, TransactionDataAPI},
};

use crate::TransactionBuilder;

impl TransactionBuilder {
    /// Add stake to a validator's staking pool using multiple IOTA coins.
    pub async fn request_add_stake(
        &self,
        signer: IotaAddress,
        mut coins: Vec<ObjectId>,
        amount: impl Into<Option<u64>>,
        validator: IotaAddress,
        gas: impl Into<Option<ObjectId>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, coins.clone(), gas_price)
            .await?;

        let mut obj_vec = vec![];
        let coin = coins
            .pop()
            .ok_or_else(|| anyhow!("Coins input should contain at lease one coin object."))?;
        let (oref, coin_type) = self.get_object_ref_and_type(coin).await?;

        let ObjectType::Struct(type_) = &coin_type else {
            bail!("Provided object [{coin}] is not a move object.");
        };
        ensure!(
            type_.is_coin(),
            "Expecting either Coin<T> input coin objects. Received [{type_}]"
        );

        for coin in coins {
            let (oref, type_) = self.get_object_ref_and_type(coin).await?;
            ensure!(
                type_ == coin_type,
                "All coins should be the same type, expecting {coin_type}, got {type_}."
            );
            obj_vec.push(CallArg::ImmutableOrOwned(oref))
        }
        obj_vec.push(CallArg::ImmutableOrOwned(oref));

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let arguments = vec![
                builder.input(CallArg::IOTA_SYSTEM_MUTABLE).unwrap(),
                builder.make_obj_vec(obj_vec)?,
                builder.pure(amount.into()).unwrap(),
                builder.pure(validator).unwrap(),
            ];
            builder.command(Command::new_move_call(
                ObjectId::SYSTEM,
                Identifier::IOTA_SYSTEM_MODULE,
                ADD_STAKE_MUL_COIN_FUN_NAME,
                vec![],
                arguments,
            ));
            builder.finish()
        };
        Ok(TransactionData::new_programmable(
            signer,
            vec![gas],
            pt,
            gas_budget,
            gas_price,
        ))
    }

    /// Withdraw stake from a validator's staking pool.
    pub async fn request_withdraw_stake(
        &self,
        signer: IotaAddress,
        staked_iota: ObjectId,
        gas: impl Into<Option<ObjectId>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let staked_iota = self.get_object_ref(staked_iota).await?;
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, vec![], gas_price)
            .await?;
        TransactionData::new_move_call(
            signer,
            ObjectId::SYSTEM,
            Identifier::IOTA_SYSTEM_MODULE,
            WITHDRAW_STAKE_FUN_NAME,
            vec![],
            gas,
            vec![
                CallArg::IOTA_SYSTEM_MUTABLE,
                CallArg::ImmutableOrOwned(staked_iota),
            ],
            gas_budget,
            gas_price,
        )
    }

    /// Add stake to a validator's staking pool using a timelocked IOTA coin.
    pub async fn request_add_timelocked_stake(
        &self,
        signer: IotaAddress,
        locked_balance: ObjectId,
        validator: IotaAddress,
        gas: ObjectId,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, Some(gas), gas_budget, vec![], gas_price)
            .await?;

        let (oref, locked_balance_type) = self.get_object_ref_and_type(locked_balance).await?;

        let ObjectType::Struct(type_) = &locked_balance_type else {
            bail!("Provided object [{locked_balance}] is not a move object.");
        };
        ensure!(
            type_.is_timelocked_balance(),
            "Expecting either TimeLock<Balance<T>> input objects. Received [{type_}]"
        );

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let arguments = vec![
                builder.input(CallArg::IOTA_SYSTEM_MUTABLE)?,
                builder.input(CallArg::ImmutableOrOwned(oref))?,
                builder.pure(validator)?,
            ];
            builder.command(Command::new_move_call(
                ObjectId::SYSTEM,
                Identifier::TIMELOCKED_STAKING_MODULE,
                ADD_TIMELOCKED_STAKE_FUN_NAME,
                vec![],
                arguments,
            ));
            builder.finish()
        };
        Ok(TransactionData::new_programmable(
            signer,
            vec![gas],
            pt,
            gas_budget,
            gas_price,
        ))
    }

    /// Withdraw timelocked stake from a validator's staking pool.
    pub async fn request_withdraw_timelocked_stake(
        &self,
        signer: IotaAddress,
        timelocked_staked_iota: ObjectId,
        gas: ObjectId,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let timelocked_staked_iota = self.get_object_ref(timelocked_staked_iota).await?;
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, Some(gas), gas_budget, vec![], gas_price)
            .await?;
        TransactionData::new_move_call(
            signer,
            ObjectId::SYSTEM,
            Identifier::TIMELOCKED_STAKING_MODULE,
            WITHDRAW_TIMELOCKED_STAKE_FUN_NAME,
            vec![],
            gas,
            vec![
                CallArg::IOTA_SYSTEM_MUTABLE,
                CallArg::ImmutableOrOwned(timelocked_staked_iota),
            ],
            gas_budget,
            gas_price,
        )
    }
}
