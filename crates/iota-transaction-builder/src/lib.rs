// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod stake; 
pub mod utils; 
pub mod package; 

use std::{collections::BTreeMap, result::Result, str::FromStr, sync::Arc};

use anyhow::{Ok, anyhow, bail, ensure};
use async_trait::async_trait;
use futures::future::join_all;
use iota_json::{
    IotaJsonValue, ResolvedCallArg, is_receiving_argument, resolve_move_function_args,
};
use iota_json_rpc_types::{
    IotaData, IotaObjectDataOptions, IotaObjectResponse, IotaRawData, IotaTypeTag,
    RPCTransactionRequestParams,
};
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID,
    base_types::{IotaAddress, ObjectID, ObjectInfo, ObjectRef, ObjectType},
    coin,
    error::UserInputError,
    fp_ensure,
    gas_coin::GasCoin,
    governance::{ADD_STAKE_MUL_COIN_FUN_NAME, WITHDRAW_STAKE_FUN_NAME},
    iota_system_state::IOTA_SYSTEM_MODULE_NAME,
    move_package::MovePackage,
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    timelock::timelocked_staking::{
        ADD_TIMELOCKED_STAKE_FUN_NAME, TIMELOCKED_STAKING_MODULE_NAME,
        WITHDRAW_TIMELOCKED_STAKE_FUN_NAME,
    },
    transaction::{
        Argument, CallArg, Command, InputObjectKind, ObjectArg, TransactionData, TransactionKind,
    },
};
use move_binary_format::{
    CompiledModule, binary_config::BinaryConfig, file_format::SignatureToken,
};
use move_core_types::{
    ident_str,
    identifier::Identifier,
    language_storage::{StructTag, TypeTag},
};
use stake::TransactionBuilder::{request_add_stake, request_withdraw_stake, request_add_timelocked_stake, request_withdraw_timelocked_stake};
use utils::TransactionBuilder::{select_gas, get_object_arg, input_refs, resolve_and_checks_json_args, get_object_ref, get_object_ref_and_type};
use package::TransactionBuilder::{publish_tx_kind, publish, upgrade_tx_kind, upgrade};

#[async_trait]
pub trait DataReader {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        object_type: StructTag,
    ) -> Result<Vec<ObjectInfo>, anyhow::Error>;

    async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error>;

    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error>;
}

#[derive(Clone)]
pub struct TransactionBuilder(Arc<dyn DataReader + Sync + Send>);

impl TransactionBuilder {
    pub fn new(data_reader: Arc<dyn DataReader + Sync + Send>) -> Self {
        Self(data_reader)
    }

    /// Construct the transaction data for a dry run
    pub async fn tx_data_for_dry_run(
        &self,
        sender: IotaAddress,
        kind: TransactionKind,
        gas_budget: u64,
        gas_price: u64,
        gas_payment: impl Into<Option<Vec<ObjectID>>>,
        gas_sponsor: impl Into<Option<IotaAddress>>,
    ) -> TransactionData {
        let gas_payment = self
            .input_refs(gas_payment.into().unwrap_or_default().as_ref())
            .await
            .unwrap_or_default();
        let gas_sponsor = gas_sponsor.into().unwrap_or(sender);
        TransactionData::new_with_gas_coins_allow_sponsor(
            kind,
            sender,
            gas_payment,
            gas_budget,
            gas_price,
            gas_sponsor,
        )
    }

    /// Construct the transaction data from a transaction kind, and other
    /// parameters. If the gas_payment list is empty, it will pick the first
    /// gas coin that has at least the required gas budget that is not in
    /// the input coins.
    pub async fn tx_data(
        &self,
        sender: IotaAddress,
        kind: TransactionKind,
        gas_budget: u64,
        gas_price: u64,
        gas_payment: Vec<ObjectID>,
        gas_sponsor: impl Into<Option<IotaAddress>>,
    ) -> Result<TransactionData, anyhow::Error> {
        let gas_payment = if gas_payment.is_empty() {
            let input_objs = kind
                .input_objects()?
                .iter()
                .flat_map(|obj| match obj {
                    InputObjectKind::ImmOrOwnedMoveObject((id, _, _)) => Some(*id),
                    _ => None,
                })
                .collect();
            vec![
                self.select_gas(sender, None, gas_budget, input_objs, gas_price)
                    .await?,
            ]
        } else {
            self.input_refs(&gas_payment).await?
        };
        Ok(TransactionData::new_with_gas_coins_allow_sponsor(
            kind,
            sender,
            gas_payment,
            gas_budget,
            gas_price,
            gas_sponsor.into().unwrap_or(sender),
        ))
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains a
    /// [`Command::TransferObjects`].
    pub async fn transfer_object_tx_kind(
        &self,
        object_id: ObjectID,
        recipient: IotaAddress,
    ) -> Result<TransactionKind, anyhow::Error> {
        let obj_ref = self.get_object_ref(object_id).await?;
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(recipient, obj_ref)?;
        Ok(TransactionKind::programmable(builder.finish()))
    }

    /// Transfer an object to the specified recipient address.
    pub async fn transfer_object(
        &self,
        signer: IotaAddress,
        object_id: ObjectID,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
        recipient: IotaAddress,
    ) -> anyhow::Result<TransactionData> {
        let mut builder = ProgrammableTransactionBuilder::new();
        self.single_transfer_object(&mut builder, object_id, recipient)
            .await?;
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, vec![object_id], gas_price)
            .await?;

        Ok(TransactionData::new(
            TransactionKind::programmable(builder.finish()),
            signer,
            gas,
            gas_budget,
            gas_price,
        ))
    }

    /// Add a [`Command::TransferObjects`] to the provided
    /// [`ProgrammableTransactionBuilder`].
    async fn single_transfer_object(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        object_id: ObjectID,
        recipient: IotaAddress,
    ) -> anyhow::Result<()> {
        builder.transfer_object(recipient, self.get_object_ref(object_id).await?)?;
        Ok(())
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains a
    /// [`Command::SplitCoins`] if some amount is provided and then transfers
    /// the split amount or the whole gas object with
    /// [`Command::TransferObjects`] to the recipient.
    pub fn transfer_iota_tx_kind(
        &self,
        recipient: IotaAddress,
        amount: impl Into<Option<u64>>,
    ) -> TransactionKind {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(recipient, amount.into());
        let pt = builder.finish();
        TransactionKind::programmable(pt)
    }

    /// Transfer IOTA from the provided coin object to the recipient address.
    /// The provided coin object is also used for the gas payment.
    pub async fn transfer_iota(
        &self,
        signer: IotaAddress,
        iota_object_id: ObjectID,
        gas_budget: u64,
        recipient: IotaAddress,
        amount: impl Into<Option<u64>>,
    ) -> anyhow::Result<TransactionData> {
        let object = self.get_object_ref(iota_object_id).await?;
        let gas_price = self.0.get_reference_gas_price().await?;
        Ok(TransactionData::new_transfer_iota(
            recipient,
            signer,
            amount.into(),
            object,
            gas_budget,
            gas_price,
        ))
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains a
    /// [`Command::MergeCoins`] if multiple inputs coins are provided and then a
    /// [`Command::SplitCoins`] together with [`Command::TransferObjects`] for
    /// each recipient + amount.
    /// The length of the vectors for recipients and amounts must be the same.
    pub async fn pay_tx_kind(
        &self,
        input_coins: Vec<ObjectID>,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let mut builder = ProgrammableTransactionBuilder::new();
        let coins = self.input_refs(&input_coins).await?;
        builder.pay(coins, recipients, amounts)?;
        let pt = builder.finish();
        Ok(TransactionKind::programmable(pt))
    }

    /// Take multiple coins and send to multiple addresses following the
    /// specified amount list. The length of the vectors must be the same.
    /// Take any type of coin, including IOTA.
    /// A separate IOTA object will be used for gas payment.
    ///
    /// If the recipient and sender are the same, it's effectively a
    /// generalized version of `split_coin` and `merge_coin`.
    pub async fn pay(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let gas = gas.into();

        if let Some(gas) = gas {
            if input_coins.contains(&gas) {
                return Err(anyhow!(
                    "Gas coin is in input coins of Pay transaction, use PayIota transaction instead!"
                ));
            }
        }

        let coin_refs = self.input_refs(&input_coins).await?;
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, input_coins, gas_price)
            .await?;

        TransactionData::new_pay(
            signer, coin_refs, recipients, amounts, gas, gas_budget, gas_price,
        )
    }

    /// Get the object references for a list of object IDs
    pub async fn input_refs(&self, obj_ids: &[ObjectID]) -> Result<Vec<ObjectRef>, anyhow::Error> {
        let handles: Vec<_> = obj_ids.iter().map(|id| self.get_object_ref(*id)).collect();
        let obj_refs = join_all(handles)
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<ObjectRef>>>()?;
        Ok(obj_refs)
    }

    /// Construct a transaction kind for the PayIota transaction type.
    ///
    /// Use this function together with tx_data_for_dry_run or tx_data
    /// for maximum reusability.
    /// The length of the vectors must be the same.
    pub fn pay_iota_tx_kind(
        &self,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_iota(recipients.clone(), amounts.clone())?;
        let pt = builder.finish();
        let tx_kind = TransactionKind::programmable(pt);
        Ok(tx_kind)
    }

    /// Take multiple IOTA coins and send to multiple addresses following the
    /// specified amount list. The length of the vectors must be the same.
    /// Only takes IOTA coins and does not require a gas coin object.
    ///
    /// The first IOTA coin object input will be used for gas payment, so the
    /// balance of this IOTA coin has to be equal to or greater than the gas
    /// budget.
    /// The total IOTA coin balance input must be sufficient to cover both the
    /// gas budget and the amounts to be transferred.
    pub async fn pay_iota(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        fp_ensure!(
            !input_coins.is_empty(),
            UserInputError::EmptyInputCoins.into()
        );

        let mut coin_refs = self.input_refs(&input_coins).await?;
        // [0] is safe because input_coins is non-empty and coins are of same length as
        // input_coins.
        let gas_object_ref = coin_refs.remove(0);
        let gas_price = self.0.get_reference_gas_price().await?;
        TransactionData::new_pay_iota(
            signer,
            coin_refs,
            recipients,
            amounts,
            gas_object_ref,
            gas_budget,
            gas_price,
        )
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains a
    /// [`Command::TransferObjects`] that sends the gas coin to the recipient.
    pub fn pay_all_iota_tx_kind(&self, recipient: IotaAddress) -> TransactionKind {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_all_iota(recipient);
        let pt = builder.finish();
        TransactionKind::programmable(pt)
    }

    /// Take multiple IOTA coins and send them to one recipient, after gas
    /// payment deduction. After the transaction, strictly zero of the IOTA
    /// coins input will be left under the sender’s address.
    ///
    /// The first IOTA coin object input will be used for gas payment, so the
    /// balance of this IOTA coin has to be equal or greater than the gas
    /// budget.
    /// A sender can transfer all their IOTA coins to another
    /// address with strictly zero IOTA left in one transaction via this
    /// transaction type.
    pub async fn pay_all_iota(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipient: IotaAddress,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        fp_ensure!(
            !input_coins.is_empty(),
            UserInputError::EmptyInputCoins.into()
        );

        let mut coin_refs = self.input_refs(&input_coins).await?;
        // [0] is safe because input_coins is non-empty and coins are of same length as
        // input_coins.
        let gas_object_ref = coin_refs.remove(0);
        let gas_price = self.0.get_reference_gas_price().await?;
        Ok(TransactionData::new_pay_all_iota(
            signer,
            coin_refs,
            recipient,
            gas_object_ref,
            gas_budget,
            gas_price,
        ))
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains a
    /// [`Command::MoveCall`].
    pub async fn move_call_tx_kind(
        &self,
        package_object_id: ObjectID,
        module: &str,
        function: &str,
        type_args: Vec<IotaTypeTag>,
        call_args: Vec<IotaJsonValue>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let mut builder = ProgrammableTransactionBuilder::new();
        self.single_move_call(
            &mut builder,
            package_object_id,
            module,
            function,
            type_args,
            call_args,
        )
        .await?;
        let pt = builder.finish();
        Ok(TransactionKind::programmable(pt))
    }

    /// Call a move function from a published package.
    pub async fn move_call(
        &self,
        signer: IotaAddress,
        package_object_id: ObjectID,
        module: &str,
        function: &str,
        type_args: Vec<IotaTypeTag>,
        call_args: Vec<IotaJsonValue>,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
        gas_price: impl Into<Option<u64>>,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = gas_price.into();

        let mut builder = ProgrammableTransactionBuilder::new();
        self.single_move_call(
            &mut builder,
            package_object_id,
            module,
            function,
            type_args,
            call_args,
        )
        .await?;
        let pt = builder.finish();
        let input_objects = pt
            .input_objects()?
            .iter()
            .flat_map(|obj| match obj {
                InputObjectKind::ImmOrOwnedMoveObject((id, _, _)) => Some(*id),
                _ => None,
            })
            .collect();
        let gas_price = if let Some(gas_price) = gas_price {
            gas_price
        } else {
            self.0.get_reference_gas_price().await?
        };
        let gas = self
            .select_gas(signer, gas, gas_budget, input_objects, gas_price)
            .await?;

        Ok(TransactionData::new(
            TransactionKind::programmable(pt),
            signer,
            gas,
            gas_budget,
            gas_price,
        ))
    }

    /// Add a single move call to the provided
    /// [`ProgrammableTransactionBuilder`].
    pub async fn single_move_call(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        package: ObjectID,
        module: &str,
        function: &str,
        type_args: Vec<IotaTypeTag>,
        call_args: Vec<IotaJsonValue>,
    ) -> anyhow::Result<()> {
        let module = Identifier::from_str(module)?;
        let function = Identifier::from_str(function)?;

        let type_args = type_args
            .into_iter()
            .map(|ty| ty.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        let call_args = self
            .resolve_and_checks_json_args(
                builder, package, &module, &function, &type_args, call_args,
            )
            .await?;

        builder.command(Command::move_call(
            package, module, function, type_args, call_args,
        ));
        Ok(())
    }

    /// Construct a transaction kind for the SplitCoin transaction type
    /// It expects that only one of the two: split_amounts or split_count is
    /// provided If both are provided, it will use split_amounts.
    pub async fn split_coin_tx_kind(
        &self,
        coin_object_id: ObjectID,
        split_amounts: impl Into<Option<Vec<u64>>>,
        split_count: impl Into<Option<u64>>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let split_amounts = split_amounts.into();
        let split_count = split_count.into();

        if split_amounts.is_none() && split_count.is_none() {
            bail!(
                "Either split_amounts or split_count must be provided for split_coin transaction."
            );
        }
        let coin = self
            .0
            .get_object_with_options(coin_object_id, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let coin_object_ref = coin.object_ref();
        let coin: Object = coin.try_into()?;
        let type_args = vec![coin.get_move_template_type()?];
        let package = IOTA_FRAMEWORK_PACKAGE_ID;
        let module = coin::PAY_MODULE_NAME.to_owned();

        let (arguments, function) = if let Some(split_amounts) = split_amounts {
            (
                vec![
                    CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_object_ref)),
                    CallArg::Pure(bcs::to_bytes(&split_amounts)?),
                ],
                coin::PAY_SPLIT_VEC_FUNC_NAME.to_owned(),
            )
        } else {
            (
                vec![
                    CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_object_ref)),
                    CallArg::Pure(bcs::to_bytes(&split_count.unwrap())?),
                ],
                coin::PAY_SPLIT_N_FUNC_NAME.to_owned(),
            )
        };
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.move_call(package, module, function, type_args, arguments)?;
        let pt = builder.finish();
        let tx_kind = TransactionKind::programmable(pt);
        Ok(tx_kind)
    }

    // TODO: consolidate this with Pay transactions
    pub async fn split_coin(
        &self,
        signer: IotaAddress,
        coin_object_id: ObjectID,
        split_amounts: Vec<u64>,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let coin = self
            .0
            .get_object_with_options(coin_object_id, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let coin_object_ref = coin.object_ref();
        let coin: Object = coin.try_into()?;
        let type_args = vec![coin.get_move_template_type()?];
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, vec![coin_object_id], gas_price)
            .await?;

        TransactionData::new_move_call(
            signer,
            IOTA_FRAMEWORK_PACKAGE_ID,
            coin::PAY_MODULE_NAME.to_owned(),
            coin::PAY_SPLIT_VEC_FUNC_NAME.to_owned(),
            type_args,
            gas,
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_object_ref)),
                CallArg::Pure(bcs::to_bytes(&split_amounts)?),
            ],
            gas_budget,
            gas_price,
        )
    }

    // TODO: consolidate this with Pay transactions
    pub async fn split_coin_equal(
        &self,
        signer: IotaAddress,
        coin_object_id: ObjectID,
        split_count: u64,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let coin = self
            .0
            .get_object_with_options(coin_object_id, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let coin_object_ref = coin.object_ref();
        let coin: Object = coin.try_into()?;
        let type_args = vec![coin.get_move_template_type()?];
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, vec![coin_object_id], gas_price)
            .await?;

        TransactionData::new_move_call(
            signer,
            IOTA_FRAMEWORK_PACKAGE_ID,
            coin::PAY_MODULE_NAME.to_owned(),
            coin::PAY_SPLIT_N_FUNC_NAME.to_owned(),
            type_args,
            gas,
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_object_ref)),
                CallArg::Pure(bcs::to_bytes(&split_count)?),
            ],
            gas_budget,
            gas_price,
        )
    }

    /// Build a [`TransactionKind::ProgrammableTransaction`] that contains
    /// [`Command::MergeCoins`] with the provided coins.
    pub async fn merge_coins_tx_kind(
        &self,
        primary_coin: ObjectID,
        coin_to_merge: ObjectID,
    ) -> Result<TransactionKind, anyhow::Error> {
        let coin = self
            .0
            .get_object_with_options(primary_coin, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let primary_coin_ref = coin.object_ref();
        let coin_to_merge_ref = self.get_object_ref(coin_to_merge).await?;
        let coin: Object = coin.try_into()?;
        let type_arguments = vec![coin.get_move_template_type()?];
        let package = IOTA_FRAMEWORK_PACKAGE_ID;
        let module = coin::COIN_MODULE_NAME.to_owned();
        let function = coin::COIN_JOIN_FUNC_NAME.to_owned();
        let arguments = vec![
            CallArg::Object(ObjectArg::ImmOrOwnedObject(primary_coin_ref)),
            CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_to_merge_ref)),
        ];
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            builder.move_call(package, module, function, type_arguments, arguments)?;
            builder.finish()
        };
        let tx_kind = TransactionKind::programmable(pt);
        Ok(tx_kind)
    }

    // TODO: consolidate this with Pay transactions
    pub async fn merge_coins(
        &self,
        signer: IotaAddress,
        primary_coin: ObjectID,
        coin_to_merge: ObjectID,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let coin = self
            .0
            .get_object_with_options(primary_coin, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let primary_coin_ref = coin.object_ref();
        let coin_to_merge_ref = self.get_object_ref(coin_to_merge).await?;
        let coin: Object = coin.try_into()?;
        let type_args = vec![coin.get_move_template_type()?];
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(
                signer,
                gas,
                gas_budget,
                vec![primary_coin, coin_to_merge],
                gas_price,
            )
            .await?;

        TransactionData::new_move_call(
            signer,
            IOTA_FRAMEWORK_PACKAGE_ID,
            coin::COIN_MODULE_NAME.to_owned(),
            coin::COIN_JOIN_FUNC_NAME.to_owned(),
            type_args,
            gas,
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(primary_coin_ref)),
                CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_to_merge_ref)),
            ],
            gas_budget,
            gas_price,
        )
    }

    /// Create an unsigned batched transaction, useful for the JSON RPC.
    pub async fn batch_transaction(
        &self,
        signer: IotaAddress,
        single_transaction_params: Vec<RPCTransactionRequestParams>,
        gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        fp_ensure!(
            !single_transaction_params.is_empty(),
            UserInputError::InvalidBatchTransaction {
                error: "Batch Transaction cannot be empty".to_owned(),
            }
            .into()
        );
        let mut builder = ProgrammableTransactionBuilder::new();
        for param in single_transaction_params {
            match param {
                RPCTransactionRequestParams::TransferObjectRequestParams(param) => {
                    self.single_transfer_object(&mut builder, param.object_id, param.recipient)
                        .await?
                }
                RPCTransactionRequestParams::MoveCallRequestParams(param) => {
                    self.single_move_call(
                        &mut builder,
                        param.package_object_id,
                        &param.module,
                        &param.function,
                        param.type_arguments,
                        param.arguments,
                    )
                    .await?
                }
            };
        }
        let pt = builder.finish();
        let all_inputs = pt.input_objects()?;
        let inputs = all_inputs
            .iter()
            .flat_map(|obj| match obj {
                InputObjectKind::ImmOrOwnedMoveObject((id, _, _)) => Some(*id),
                _ => None,
            })
            .collect();
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(signer, gas, gas_budget, inputs, gas_price)
            .await?;

        Ok(TransactionData::new(
            TransactionKind::programmable(pt),
            signer,
            gas,
            gas_budget,
            gas_price,
        ))
    }

}
