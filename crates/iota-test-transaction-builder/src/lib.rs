// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use iota_genesis_builder::validator_info::GenesisValidatorMetadata;
use iota_move_build::{BuildConfig, CompiledPackage};
use iota_sdk::{
    rpc_types::{
        IotaObjectDataOptions, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
        get_new_package_obj_from_response,
    },
    wallet_context::WalletContext,
};
use iota_sdk_crypto::Signer as SdkSigner;
use iota_sdk_transaction_builder::{PTBArgumentList, TransactionBuilder};
use iota_sdk_types::{
    Address, Identifier, Input, ObjectId, ObjectReference, Owner, ProgrammableTransaction,
    StructTag, TransactionKind, TypeTag, Version,
    crypto::{Intent, IntentMessage, SimpleSignature},
};
use iota_types::{
    crypto::{AccountKeyPair, IotaKeyPair, get_key_pair},
    digests::TransactionDigest,
    multisig::{BitmapUnit, MultiSig, MultiSigPublicKey},
    signature::GenericSignature,
    transaction::{
        CallArg, DEFAULT_VALIDATOR_GAS_PRICE, SharedObjectRef,
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        Transaction, TransactionData, TransactionDataAPI,
    },
    utils::to_sender_signed_transaction,
};
use rand::Rng;

pub struct TestTransactionBuilder {
    test_data: TestTransactionData,
    sender: Address,
    gas_object: ObjectReference,
    gas_price: u64,
    gas_budget: Option<u64>,
    nonce: Option<u64>,
}

impl TestTransactionBuilder {
    pub fn new(sender: Address, gas_object: ObjectReference, gas_price: u64) -> Self {
        Self {
            test_data: TestTransactionData::Empty,
            sender,
            gas_object,
            gas_price,
            gas_budget: None,
            nonce: None,
        }
    }

    /// Inject a random unused pure input so that two otherwise-identical
    /// transactions build to distinct digests.
    ///
    /// Use this for workloads that repeatedly submit logically identical
    /// transactions (same sender, gas object and arguments) and must avoid
    /// colliding on an already-executed digest.
    pub fn ensure_unique(mut self) -> Self {
        self.nonce = Some(rand::thread_rng().gen());
        self
    }

    pub fn sender(&self) -> Address {
        self.sender
    }

    pub fn gas_object(&self) -> ObjectReference {
        self.gas_object
    }

    // Use `with_type_args` below to provide type args if any
    pub fn move_call(
        mut self,
        package_id: ObjectId,
        module: &str,
        function: &str,
        args: Vec<CallArg>,
    ) -> Self {
        assert!(matches!(self.test_data, TestTransactionData::Empty));
        self.test_data = TestTransactionData::Move(MoveData {
            package_id,
            module: Identifier::new(module).unwrap(),
            function: Identifier::new(function).unwrap(),
            args,
            type_args: vec![],
        });
        self
    }

    pub fn with_type_args(mut self, type_args: Vec<TypeTag>) -> Self {
        if let TestTransactionData::Move(data) = &mut self.test_data {
            assert!(data.type_args.is_empty());
            data.type_args = type_args;
        } else {
            panic!("Cannot set type args for non-move call");
        }
        self
    }

    pub fn with_gas_budget(mut self, gas_budget: u64) -> Self {
        self.gas_budget = Some(gas_budget);
        self
    }

    pub fn call_counter_create(self, package_id: ObjectId) -> Self {
        self.move_call(package_id, "counter", "create", vec![])
    }

    pub fn call_counter_increment(
        self,
        package_id: ObjectId,
        counter_id: ObjectId,
        counter_initial_shared_version: Version,
    ) -> Self {
        self.move_call(
            package_id,
            "counter",
            "increment",
            vec![CallArg::Shared(SharedObjectRef::new(
                counter_id,
                counter_initial_shared_version,
                true,
            ))],
        )
    }

    pub fn call_counter_read(
        self,
        package_id: ObjectId,
        counter_id: ObjectId,
        counter_initial_shared_version: Version,
    ) -> Self {
        self.move_call(
            package_id,
            "counter",
            "value",
            vec![CallArg::Shared(SharedObjectRef::new(
                counter_id,
                counter_initial_shared_version,
                false,
            ))],
        )
    }

    pub fn call_counter_delete(
        self,
        package_id: ObjectId,
        counter_id: ObjectId,
        counter_initial_shared_version: Version,
    ) -> Self {
        self.move_call(
            package_id,
            "counter",
            "delete",
            vec![CallArg::Shared(SharedObjectRef::new(
                counter_id,
                counter_initial_shared_version,
                true,
            ))],
        )
    }

    pub fn call_nft_create(self, package_id: ObjectId) -> Self {
        self.move_call(
            package_id,
            "testnet_nft",
            "mint_to_sender",
            vec![
                CallArg::pure(&"example_nft_name"),
                CallArg::pure(&"example_nft_description"),
                CallArg::pure(&"https://iota.org/_nuxt/img/iota-logo.8d3c44e.svg"),
            ],
        )
    }

    pub fn call_nft_delete(self, package_id: ObjectId, nft_to_delete: ObjectReference) -> Self {
        self.move_call(
            package_id,
            "testnet_nft",
            "burn",
            vec![CallArg::ImmutableOrOwned(nft_to_delete)],
        )
    }

    pub fn call_staking(self, stake_coin: ObjectReference, validator: Address) -> Self {
        self.move_call(
            ObjectId::SYSTEM,
            Identifier::IOTA_SYSTEM_MODULE.as_str(),
            "request_add_stake",
            vec![
                CallArg::IOTA_SYSTEM_MUTABLE,
                CallArg::ImmutableOrOwned(stake_coin),
                CallArg::pure(&validator),
            ],
        )
    }

    pub fn call_emit_random(
        self,
        package_id: ObjectId,
        randomness_initial_shared_version: Version,
    ) -> Self {
        self.move_call(
            package_id,
            "random",
            "new",
            vec![CallArg::Shared(SharedObjectRef::new(
                ObjectId::RANDOMNESS_STATE,
                randomness_initial_shared_version,
                false,
            ))],
        )
    }

    pub fn call_request_add_validator(self) -> Self {
        self.move_call(
            ObjectId::SYSTEM,
            Identifier::IOTA_SYSTEM_MODULE.as_str(),
            "request_add_validator",
            vec![CallArg::IOTA_SYSTEM_MUTABLE],
        )
    }

    pub fn call_request_add_validator_candidate(
        self,
        validator: &GenesisValidatorMetadata,
    ) -> Self {
        self.move_call(
            ObjectId::SYSTEM,
            Identifier::IOTA_SYSTEM_MODULE.as_str(),
            "request_add_validator_candidate",
            vec![
                CallArg::IOTA_SYSTEM_MUTABLE,
                CallArg::pure(&validator.authority_public_key),
                CallArg::pure(&validator.network_public_key),
                CallArg::pure(&validator.protocol_public_key),
                CallArg::pure(&validator.proof_of_possession),
                CallArg::pure(&validator.name),
                CallArg::pure(&validator.description),
                CallArg::pure(&validator.image_url),
                CallArg::pure(&validator.project_url),
                CallArg::pure(&validator.network_address),
                CallArg::pure(&validator.p2p_address),
                CallArg::pure(&validator.primary_address),
                CallArg::pure(&DEFAULT_VALIDATOR_GAS_PRICE), // gas_price
                CallArg::pure(&0u64),                        // commission_rate
            ],
        )
    }

    pub fn call_request_remove_validator(self) -> Self {
        self.move_call(
            ObjectId::SYSTEM,
            Identifier::IOTA_SYSTEM_MODULE.as_str(),
            "request_remove_validator",
            vec![CallArg::IOTA_SYSTEM_MUTABLE],
        )
    }

    pub fn transfer(mut self, object: ObjectReference, recipient: Address) -> Self {
        self.test_data = TestTransactionData::Transfer(TransferData { object, recipient });
        self
    }

    pub fn transfer_iota(mut self, amount: Option<u64>, recipient: Address) -> Self {
        self.test_data = TestTransactionData::TransferIota(TransferIotaData { amount, recipient });
        self
    }

    pub fn split_coin(mut self, coin: ObjectReference, amounts: Vec<u64>) -> Self {
        self.test_data = TestTransactionData::SplitCoin(SplitCoinData { coin, amounts });
        self
    }

    pub fn publish(mut self, path: PathBuf) -> Self {
        assert!(matches!(self.test_data, TestTransactionData::Empty));
        self.test_data = TestTransactionData::Publish(PublishData::Source(path, false));
        self
    }

    pub fn publish_with_deps(mut self, path: PathBuf) -> Self {
        assert!(matches!(self.test_data, TestTransactionData::Empty));
        self.test_data = TestTransactionData::Publish(PublishData::Source(path, true));
        self
    }

    pub fn publish_with_data(mut self, data: PublishData) -> Self {
        assert!(matches!(self.test_data, TestTransactionData::Empty));
        self.test_data = TestTransactionData::Publish(data);
        self
    }

    pub fn publish_examples(self, subpath: &'static str) -> Self {
        let path = if let Ok(p) = std::env::var("MOVE_EXAMPLES_DIR") {
            let mut path = PathBuf::from(p);
            path.extend([subpath]);
            path
        } else {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.extend(["..", "..", "examples", "move", subpath]);
            path
        };
        self.publish(path)
    }

    pub fn programmable(mut self, programmable: ProgrammableTransaction) -> Self {
        self.test_data = TestTransactionData::Programmable(programmable);
        self
    }

    pub fn build(self) -> TransactionData {
        let nonce = self.nonce;
        let mut data = self.build_inner();
        if let Some(nonce) = nonce {
            // A trailing pure input that no command references leaves execution
            // unchanged but alters the serialized transaction, and hence its
            // digest.
            if let TransactionKind::Programmable(pt) = data.kind_mut() {
                pt.inputs.push(Input::Pure(nonce.to_le_bytes().to_vec()));
            }
        }
        data
    }

    fn build_inner(self) -> TransactionData {
        match self.test_data {
            TestTransactionData::Move(data) => TransactionData::new_move_call(
                self.sender,
                data.package_id,
                data.module,
                data.function,
                data.type_args,
                self.gas_object,
                data.args,
                self.gas_budget
                    .unwrap_or(self.gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE),
                self.gas_price,
            )
            .unwrap(),
            TestTransactionData::Transfer(data) => TransactionData::new_transfer(
                data.recipient,
                data.object,
                self.sender,
                self.gas_object,
                self.gas_budget
                    .unwrap_or(self.gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
                self.gas_price,
            ),
            TestTransactionData::TransferIota(data) => TransactionData::new_transfer_iota(
                data.recipient,
                self.sender,
                data.amount,
                self.gas_object,
                self.gas_budget
                    .unwrap_or(self.gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
                self.gas_price,
            ),
            TestTransactionData::SplitCoin(data) => TransactionData::new_split_coin(
                self.sender,
                data.coin,
                data.amounts,
                self.gas_object,
                self.gas_budget
                    .unwrap_or(self.gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
                self.gas_price,
            ),
            TestTransactionData::Publish(data) => {
                let (all_module_bytes, dependencies) = match data {
                    PublishData::Source(path, with_unpublished_deps) => {
                        let compiled_package = BuildConfig::new_for_testing().build(&path).unwrap();
                        let all_module_bytes =
                            compiled_package.get_package_bytes(with_unpublished_deps);
                        let dependencies = compiled_package.get_dependency_storage_package_ids();
                        (all_module_bytes, dependencies)
                    }
                    PublishData::ModuleBytes(bytecode) => (bytecode, vec![]),
                    PublishData::CompiledPackage(compiled_package) => {
                        let all_module_bytes = compiled_package.get_package_bytes(false);
                        let dependencies = compiled_package.get_dependency_storage_package_ids();
                        (all_module_bytes, dependencies)
                    }
                };

                TransactionData::new_module(
                    self.sender,
                    self.gas_object,
                    all_module_bytes,
                    dependencies,
                    self.gas_budget.unwrap_or(
                        self.gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
                    ),
                    self.gas_price,
                )
            }
            TestTransactionData::Programmable(pt) => TransactionData::new_programmable(
                self.sender,
                vec![self.gas_object],
                pt,
                self.gas_budget
                    .unwrap_or(self.gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE),
                self.gas_price,
            ),
            TestTransactionData::Empty => {
                panic!("Cannot build empty transaction");
            }
        }
    }

    pub fn build_and_sign(self, signer: impl Into<IotaKeyPair>) -> Transaction {
        Transaction::from_data_and_signer(self.build(), vec![signer])
    }

    pub fn build_and_sign_multisig(
        self,
        multisig_pk: MultiSigPublicKey,
        signers: &[&dyn SdkSigner<SimpleSignature>],
        bitmap: BitmapUnit,
    ) -> Transaction {
        let data = self.build();
        let digest = IntentMessage::new(Intent::iota_transaction(), data.clone()).signing_digest();
        let signatures = signers.iter().map(|s| s.sign(&*digest).into()).collect();
        let multisig =
            GenericSignature::MultiSig(MultiSig::new_unchecked(signatures, bitmap, multisig_pk));

        Transaction::from_generic_sig_data(data, vec![multisig])
    }
}

#[expect(clippy::large_enum_variant)]
enum TestTransactionData {
    Move(MoveData),
    Transfer(TransferData),
    TransferIota(TransferIotaData),
    SplitCoin(SplitCoinData),
    Publish(PublishData),
    Programmable(ProgrammableTransaction),
    Empty,
}

struct MoveData {
    package_id: ObjectId,
    module: Identifier,
    function: Identifier,
    args: Vec<CallArg>,
    type_args: Vec<TypeTag>,
}

#[expect(clippy::large_enum_variant)]
pub enum PublishData {
    /// Path to source code directory and with_unpublished_deps.
    /// with_unpublished_deps indicates whether to publish unpublished
    /// dependencies in the same transaction or not.
    Source(PathBuf, bool),
    ModuleBytes(Vec<Vec<u8>>),
    CompiledPackage(CompiledPackage),
}

struct TransferData {
    object: ObjectReference,
    recipient: Address,
}

struct TransferIotaData {
    amount: Option<u64>,
    recipient: Address,
}

struct SplitCoinData {
    coin: ObjectReference,
    amounts: Vec<u64>,
}

/// A helper function to make Transactions with controlled accounts in
/// WalletContext. Particularly, the wallet needs to own gas objects for
/// transactions. However, if this function is called multiple times without any
/// "sync" actions on gas object management, txns may fail and objects may be
/// locked.
///
/// The param is called `max_txn_num` because it does not always return the
/// exact same amount of Transactions, for example when there are not enough gas
/// objects controlled by the WalletContext. Caller should rely on the return
/// value to check the count.
pub async fn batch_make_transfer_transactions(
    context: &WalletContext,
    max_txn_num: usize,
) -> Vec<Transaction> {
    let recipient = get_key_pair::<AccountKeyPair>().0;
    let result = context.get_all_accounts_and_gas_objects().await;
    let accounts_and_objs = result.unwrap();
    let mut res = Vec::with_capacity(max_txn_num);

    let gas_price = context.get_reference_gas_price().await.unwrap();
    for (address, objs) in accounts_and_objs {
        for obj in objs {
            if res.len() >= max_txn_num {
                return res;
            }
            let data = TransactionData::new_transfer_iota(
                recipient,
                address,
                Some(2),
                obj,
                gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                gas_price,
            );
            let tx = context.sign_transaction(&data);
            res.push(tx);
        }
    }
    res
}

pub async fn make_transfer_iota_transaction(
    context: &WalletContext,
    recipient: Option<Address>,
    amount: Option<u64>,
) -> Transaction {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .transfer_iota(amount, recipient.unwrap_or(sender))
            .build(),
    )
}

pub async fn make_staking_transaction(
    context: &WalletContext,
    validator_address: Address,
) -> Transaction {
    let accounts_and_objs = context.get_all_accounts_and_gas_objects().await.unwrap();
    let sender = accounts_and_objs[0].0;
    let gas_object = accounts_and_objs[0].1[0];
    let stake_object = accounts_and_objs[0].1[1];
    let gas_price = context.get_reference_gas_price().await.unwrap();
    context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .call_staking(stake_object, validator_address)
            .build(),
    )
}

pub async fn make_publish_transaction(context: &WalletContext, path: PathBuf) -> Transaction {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish(path)
            .build(),
    )
}

pub async fn make_publish_transaction_with_deps(
    context: &WalletContext,
    path: PathBuf,
) -> Transaction {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish_with_deps(path)
            .build(),
    )
}

pub async fn publish_package(context: &WalletContext, path: PathBuf) -> ObjectReference {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish(path)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    get_new_package_obj_from_response(&resp).unwrap()
}

/// Executes a transaction to publish the `basics` package and returns the
/// package object ref.
pub async fn publish_basics_package(context: &WalletContext) -> ObjectReference {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish_examples("basics")
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    get_new_package_obj_from_response(&resp).unwrap()
}

/// Executes a transaction to publish the `basics` package and another one to
/// create a counter. Returns the package object ref and the counter object ref.
pub async fn publish_basics_package_and_make_counter(
    context: &WalletContext,
) -> (ObjectReference, ObjectReference) {
    let package_ref = publish_basics_package(context).await;
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let counter_creation_txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .call_counter_create(package_ref.object_id)
            .build(),
    );
    let resp = context
        .execute_transaction_must_succeed(counter_creation_txn)
        .await;
    let counter_ref = resp
        .effects
        .unwrap()
        .created()
        .iter()
        .find(|obj_ref| matches!(obj_ref.owner, Owner::Shared(_)))
        .unwrap()
        .reference;
    (package_ref, counter_ref)
}

/// Executes a transaction to increment a counter object.
/// Must be called after calling `publish_basics_package_and_make_counter`.
pub async fn increment_counter(
    context: &WalletContext,
    sender: Address,
    gas_object_id: Option<ObjectId>,
    package_id: ObjectId,
    counter_id: ObjectId,
    initial_shared_version: Version,
) -> IotaTransactionBlockResponse {
    let gas_object = if let Some(gas_object_id) = gas_object_id {
        context.get_object_ref(gas_object_id).await.unwrap()
    } else {
        context
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap()
    };
    let rgp = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, rgp)
            .call_counter_increment(package_id, counter_id, initial_shared_version)
            .build(),
    );
    context.execute_transaction_must_succeed(txn).await
}

/// Executes a transaction that generates a new random u128 using Random and
/// emits it as an event.
pub async fn emit_new_random_u128(
    context: &WalletContext,
    package_id: ObjectId,
) -> IotaTransactionBlockResponse {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let rgp = context.get_reference_gas_price().await.unwrap();

    let client = context.get_client().await.unwrap();
    let random_obj = client
        .read_api()
        .get_object_with_options(
            ObjectId::RANDOMNESS_STATE,
            IotaObjectDataOptions::new().with_owner(),
        )
        .await
        .unwrap()
        .into_object()
        .unwrap();
    let random_obj_owner = random_obj
        .owner
        .expect("Expect Randomness object to have an owner");

    let Owner::Shared(initial_shared_version) = random_obj_owner else {
        panic!("Expect Randomness to be shared object")
    };
    let random_call_arg = CallArg::Shared(SharedObjectRef::new(
        ObjectId::RANDOMNESS_STATE,
        initial_shared_version,
        false,
    ));

    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, rgp)
            .move_call(package_id, "random", "new", vec![random_call_arg])
            .build(),
    );
    context.execute_transaction_must_succeed(txn).await
}

/// Executes a transaction to publish the specified examples package and returns
/// the package id and the digest of the transaction.
pub async fn publish_example_package(
    context: &WalletContext,
    example_subpath: &'static str,
    sender_key_pair: &AccountKeyPair,
    sender: Address,
    gas: ObjectReference,
) -> (ObjectId, TransactionDigest) {
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let tx = to_sender_signed_transaction(
        TestTransactionBuilder::new(sender, gas, gas_price)
            .publish_examples(example_subpath)
            .build(),
        sender_key_pair,
    );

    let resp = context.execute_transaction_must_succeed(tx).await;
    let package_id = get_new_package_obj_from_response(&resp).unwrap().object_id;
    (package_id, resp.digest)
}

/// Executes a transaction to publish the `nft` package and returns the package
/// id, id of the gas object used, and the digest of the transaction.
pub async fn publish_nfts_package(
    context: &WalletContext,
) -> (ObjectId, ObjectId, TransactionDigest) {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_id = gas_object.object_id;
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish_examples("nft")
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    let package_id = get_new_package_obj_from_response(&resp).unwrap().object_id;
    (package_id, gas_id, resp.digest)
}

/// Executes a transaction to publish the `simple_warrior` package and returns
/// the package id and the digest of the transaction.
pub async fn publish_simple_warrior_package(
    context: &WalletContext,
    sender_key_pair: &AccountKeyPair,
    sender: Address,
    gas: ObjectReference,
) -> (ObjectId, TransactionDigest) {
    publish_example_package(context, "simple_warrior", sender_key_pair, sender, gas).await
}

/// Pre-requisite: `publish_nfts_package` must be called before this function.
/// Executes a transaction to create an NFT and returns the sender address, the
/// object id of the NFT, and the digest of the transaction.
pub async fn create_nft(
    context: &WalletContext,
    package_id: ObjectId,
) -> (Address, ObjectId, TransactionDigest) {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let rgp = context.get_reference_gas_price().await.unwrap();

    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, rgp)
            .call_nft_create(package_id)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;

    let object_id = resp
        .effects
        .as_ref()
        .unwrap()
        .created()
        .first()
        .unwrap()
        .reference
        .object_id;

    (sender, object_id, resp.digest)
}

/// Executes a transaction to delete the given NFT.
pub async fn delete_nft(
    context: &WalletContext,
    sender: Address,
    package_id: ObjectId,
    nft_to_delete: ObjectReference,
) -> IotaTransactionBlockResponse {
    let gas = context
        .get_one_gas_object_owned_by_address(sender)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("Expect {sender} to have at least one gas object"));
    let rgp = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas, rgp)
            .call_nft_delete(package_id, nft_to_delete)
            .build(),
    );
    context.execute_transaction_must_succeed(txn).await
}

/// Fetch one IOTA coin owned by `sender` to use as an explicit gas coin.
///
/// Without an explicit gas coin, [`TransactionBuilder::finish`] auto-adds
/// every IOTA coin the sender owns as gas inputs and merges the leftover into
/// one output coin, breaking tests that observe the sender's coin count.
pub async fn select_gas_coin(grpc_client: &iota_grpc_client::Client, sender: Address) -> ObjectId {
    let gas_coin = grpc_client
        .list_owned_objects(sender, Some(StructTag::new_gas_coin()), Some(1), None, None)
        .collect(Some(1))
        .await
        .expect("failed to fetch gas coin")
        .into_inner()
        .into_iter()
        .next()
        .expect("sender has no gas coin");
    *gas_coin
        .object_reference()
        .expect("gas coin missing object reference")
        .object_id()
}

/// Build a Move-call transaction ready to be signed, paying gas from a single
/// coin picked with [`select_gas_coin`].
///
/// `args` is anything the builder accepts as an argument list: a tuple of
/// mixed argument types (`ObjectId` for owned objects, `SharedMut(id)` for
/// shared mutable objects, `u64`/`Address` and other pure values), or an
/// array/`Vec` of a single argument type.
pub async fn move_call_tx<A: PTBArgumentList>(
    grpc_client: &iota_grpc_client::Client,
    sender: Address,
    package_id: ObjectId,
    module: &str,
    function: &str,
    args: A,
    gas_budget: u64,
) -> TransactionData {
    let mut builder = TransactionBuilder::new(sender).with_client(grpc_client);

    builder
        .move_call(package_id, module, function)
        .arguments(args);

    builder.gas(vec![select_gas_coin(grpc_client, sender).await]);
    builder.gas_budget(gas_budget);

    builder
        .finish()
        .await
        .expect("failed to construct move call transaction")
}

/// Build a transaction splitting `coin_to_split` into `num_coins` coins of
/// equal value, ready to be signed. The original coin keeps the remainder.
///
/// `gas_coin` must differ from `coin_to_split`; when `None`, the builder
/// selects gas automatically from the sender's IOTA coins.
pub async fn split_coin_equal_tx(
    grpc_client: &iota_grpc_client::Client,
    sender: Address,
    coin_to_split: ObjectId,
    num_coins: u64,
    gas_coin: Option<ObjectId>,
    gas_budget: u64,
) -> TransactionData {
    let coin_object = grpc_client
        .get_objects(&[(coin_to_split, None)], None)
        .await
        .expect("failed to fetch coin")
        .into_inner()
        .into_iter()
        .next()
        .expect("coin not found")
        .object()
        .expect("invalid coin object");
    let coin_balance = iota_sdk_types::Coin::try_from_object(&coin_object)
        .expect("object is not a coin")
        .balance();

    // Create `num_coins - 1` new coins of equal value; the original keeps the
    // remainder.
    let amount_per_split = coin_balance / num_coins;
    let split_amounts: Vec<u64> = vec![amount_per_split; (num_coins - 1) as usize];

    let mut builder = TransactionBuilder::new(sender).with_client(grpc_client);

    // Split off the new coin and transfer it back to the sender; an untransferred
    // `Coin` would be an unused PTB value (coins have no `drop`) and the
    // transaction would be rejected.
    let new_coin = builder.split_coins(coin_to_split, split_amounts).arg();
    builder.transfer_objects(sender, [new_coin]);

    if let Some(gas) = gas_coin {
        builder.gas([gas]);
    }
    builder.gas_budget(gas_budget);

    builder
        .finish()
        .await
        .expect("failed to construct split coin transaction")
}

#[cfg(test)]
mod tests {
    use iota_types::base_types::{dbg_addr, random_object_ref};

    use super::*;

    #[test]
    fn ensure_unique_changes_digest() {
        let sender = dbg_addr(1);
        let recipient = dbg_addr(2);
        let gas = random_object_ref();
        let build =
            || TestTransactionBuilder::new(sender, gas, 1000).transfer_iota(Some(1), recipient);

        // Identical inputs build to the same digest.
        assert_eq!(build().build().digest(), build().build().digest());

        let base = build().build();
        let unique = build().ensure_unique().build();

        // ensure_unique() perturbs the digest by appending one trailing pure
        // input that no command references.
        assert_ne!(base.digest(), unique.digest());
        match (base.kind(), unique.kind()) {
            (TransactionKind::Programmable(base), TransactionKind::Programmable(unique)) => {
                assert_eq!(unique.inputs.len(), base.inputs.len() + 1);
                assert!(matches!(unique.inputs.last(), Some(Input::Pure(_))));
            }
            _ => panic!("expected programmable transactions"),
        }

        // Two independent unique builds differ from each other.
        assert_ne!(
            build().ensure_unique().build().digest(),
            build().ensure_unique().build().digest()
        );
    }
}
