// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaTransactionBlockBuilderMode, IotaTypeTag, RPCTransactionRequestParams,
    TransactionBlockBytes,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    iota_serde::BigInt,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// Provides methods for constructing transactions such as transferring objects,
/// sending coins, performing Move calls, or managing stakes.
#[open_rpc(namespace = "unsafe", tag = "Transaction Builder API")]
#[rpc(server, client, namespace = "unsafe")]
pub trait TransactionBuilder {
    /// Create an unsigned transaction to transfer an object from one address to
    /// another. The object's type must allow public transfers
    #[rustfmt::skip]
    #[method(name = "transferObject")]
    async fn transfer_object(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the ID of the object to be transferred
        object_id: ObjectID,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
        /// the recipient's IOTA address
        recipient: IotaAddress,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to send IOTA coin object to an IOTA address.
    /// The IOTA object is also used as the gas object.
    #[rustfmt::skip]
    #[method(name = "transferIota")]
    async fn transfer_iota(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the IOTA coin object to be used in this transaction
        iota_object_id: ObjectID,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
        /// the recipient's IOTA address
        recipient: IotaAddress,
        /// the amount to be split out and transferred
        amount: Option<BigInt<u64>>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Send `Coin<T>` to a list of addresses, where `T` can be any coin type,
    /// following a list of amounts, The object specified in the `gas` field
    /// will be used to pay the gas fee for the transaction. The gas object can
    /// not appear in `input_coins`. If the gas object is not specified, the RPC
    /// server will auto-select one.
    #[rustfmt::skip]
    #[method(name = "pay")]
    async fn pay(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the IOTA coins to be used in this transaction
        input_coins: Vec<ObjectID>,
        /// the recipients' addresses, the length of this vector must be the same as amounts.
        recipients: Vec<IotaAddress>,
        /// the amounts to be transferred to recipients, following the same order
        amounts: Vec<BigInt<u64>>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Send IOTA coins to a list of addresses, following a list of amounts.
    /// This is for IOTA coin only and does not require a separate gas coin object.
    /// Specifically, what pay_iota does are:
    /// 1. debit each input_coin to create new coin following the order of
    /// amounts and assign it to the corresponding recipient.
    /// 2. accumulate all residual IOTA from input coins left and deposit all IOTA
    ///    to the first
    /// input coin, then use the first input coin as the gas coin object.
    /// 3. the balance of the first input coin after tx is sum(input_coins) -
    ///    sum(amounts) - actual_gas_cost
    /// 4. all other input coints other than the first one are deleted.
    #[rustfmt::skip]
    #[method(name = "payIota")]
    async fn pay_iota(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the IOTA coins to be used in this transaction, including the coin for gas payment.
        input_coins: Vec<ObjectID>,
        /// the recipients' addresses, the length of this vector must be the same as amounts.
        recipients: Vec<IotaAddress>,
        /// the amounts to be transferred to recipients, following the same order
        amounts: Vec<BigInt<u64>>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Send all IOTA coins to one recipient.
    /// This is for IOTA coin only and does not require a separate gas coin object.
    /// Specifically, what pay_all_iota does are:
    /// 1. accumulate all IOTA from input coins and deposit all IOTA to the first
    ///    input coin
    /// 2. transfer the updated first coin to the recipient and also use this first
    ///    coin as gas coin object.
    /// 3. the balance of the first input coin after tx is sum(input_coins) -
    ///    actual_gas_cost.
    /// 4. all other input coins other than the first are deleted.
    #[rustfmt::skip]
    #[method(name = "payAllIota")]
    async fn pay_all_iota(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the IOTA coins to be used in this transaction, including the coin for gas payment.
        input_coins: Vec<ObjectID>,
        /// the recipient address,
        recipient: IotaAddress,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to execute a Move call on the network, by
    /// calling the specified function in the module of a given package.
    #[rustfmt::skip]
    #[method(name = "moveCall")]
    async fn move_call(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the Move package ID, e.g. `0x2`
        package_object_id: ObjectID,
        /// the Move module name, e.g. `pay`
        module: String,
        /// the move function name, e.g. `split`
        function: String,
        /// the type arguments of the Move function
        type_arguments: Vec<IotaTypeTag>,
        /// the arguments to be passed into the Move function, in [IotaJson](https://docs.iota.org/references/iota-api) format
        arguments: Vec<IotaJsonValue>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
        /// Whether this is a Normal transaction or a Dev Inspect Transaction. Default to be `IotaTransactionBlockBuilderMode::Commit` when it's None.
        execution_mode: Option<IotaTransactionBlockBuilderMode>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to publish a Move package.
    #[rustfmt::skip]
    #[method(name = "publish")]
    async fn publish(
        &self,
        /// the transaction signer's IOTA address
        sender: IotaAddress,
        /// the compiled bytes of a Move package
        compiled_modules: Vec<Base64>,
        /// a list of transitive dependency addresses that this set of modules depends on.
        dependencies: Vec<ObjectID>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to split a coin object into multiple
    /// coins.
    #[rustfmt::skip]
    #[method(name = "splitCoin")]
    async fn split_coin(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the coin object to be spilt
        coin_object_id: ObjectID,
        /// the amounts to split out from the coin
        split_amounts: Vec<BigInt<u64>>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to split a coin object into multiple
    /// equal-size coins.
    #[rustfmt::skip]
    #[method(name = "splitCoinEqual")]
    async fn split_coin_equal(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the coin object to be spilt
        coin_object_id: ObjectID,
        /// the number of coins to split into
        split_count: BigInt<u64>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned transaction to merge multiple coins into one coin.
    #[rustfmt::skip]
    #[method(name = "mergeCoins")]
    async fn merge_coin(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// the coin object to merge into, this coin will remain after the transaction
        primary_coin: ObjectID,
        /// the coin object to be merged, this coin will be destroyed, the balance will be added to `primary_coin`
        coin_to_merge: ObjectID,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Create an unsigned batched transaction.
    #[rustfmt::skip]
    #[method(name = "batchTransaction")]
    async fn batch_transaction(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// list of transaction request parameters
        single_transaction_params: Vec<RPCTransactionRequestParams>,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
        /// Whether this is a regular transaction or a Dev Inspect Transaction
        txn_builder_mode: Option<IotaTransactionBlockBuilderMode>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Add stake to a validator's staking pool using multiple coins and amount.
    #[rustfmt::skip]
    #[method(name = "requestAddStake")]
    async fn request_add_stake(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// Coin<IOTA> object to stake
        coins: Vec<ObjectID>,
        /// stake amount
        amount: Option<BigInt<u64>>,
        /// the validator's IOTA address
        validator: IotaAddress,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Withdraw stake from a validator's staking pool.
    #[rustfmt::skip]
    #[method(name = "requestWithdrawStake")]
    async fn request_withdraw_stake(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// StakedIota object ID
        staked_iota: ObjectID,
        /// gas object to be used in this transaction, node will pick one from the signer's possession if not provided
        gas: Option<ObjectID>,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Add timelocked stake to a validator's staking pool using multiple balances
    /// and amount.
    #[rustfmt::skip]
    #[method(name = "requestAddTimelockedStake")]
    async fn request_add_timelocked_stake(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// TimeLock<Balance<IOTA>> object to stake
        locked_balance: ObjectID,
        /// the validator's IOTA address
        validator: IotaAddress,
        /// gas object to be used in this transaction
        gas: ObjectID,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;

    /// Withdraw timelocked stake from a validator's staking pool.
    #[rustfmt::skip]
    #[method(name = "requestWithdrawTimelockedStake")]
    async fn request_withdraw_timelocked_stake(
        &self,
        /// the transaction signer's IOTA address
        signer: IotaAddress,
        /// TimelockedStakedIota object ID
        timelocked_staked_iota: ObjectID,
        /// gas object to be used in this transaction
        gas: ObjectID,
        /// the gas budget, the transaction will fail if the gas cost exceed the budget
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes>;
}
