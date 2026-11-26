// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Helpers to create a `SmartAccount` with a public key attached, to send
//! transactions from it and to change its key.

use iota_sdk_crypto::{Signer, ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
use iota_sdk_types::{
    Address, Argument, Identifier, MoveAuthenticatorV1, ObjectId, ObjectReference, Owner,
    ProgrammableTransaction, SharedObjectReference, SimpleSignature, Transaction,
    TransactionEffects, TypeTag, UserSignature, crypto::PersonalMessage,
    key_proof::key_proof_message_v1,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::builtin_authenticator_functions::PublicKeyFieldName,
    dynamic_field::derive_dynamic_field_id,
    effects::TransactionEffectsAPI,
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use test_cluster::TestCluster;

/// The last epoch in which the key proofs made by these helpers are valid.
pub const VALID_UNTIL_EPOCH: u64 = 10;

/// Returns an Ed25519 key pair derived from `seed`.
pub fn ed25519_key(seed: u8) -> SimpleKeypair {
    SimpleKeypair::from(Ed25519PrivateKey::random_with(StdRng::from_seed(
        [seed; 32],
    )))
}

/// Returns the chain identifier of `cluster`: the digest of its genesis
/// checkpoint.
pub fn chain_identifier(cluster: &TestCluster) -> [u8; 32] {
    *cluster.swarm.config().genesis.checkpoint().digest().bytes()
}

/// Signs `message` with `key` as an IOTA personal message, in the format a
/// key proof is given in.
pub fn sign_personal_message(key: &SimpleKeypair, message: &str) -> Vec<u8> {
    let signature: SimpleSignature =
        key.sign(&PersonalMessage(message.as_bytes().into()).signing_digest());
    UserSignature::Simple(signature).to_bytes()
}

/// Adds to `builder` the commands that make the `PublicKey` of `key`, and
/// returns it.
pub fn public_key_argument(
    builder: &mut ProgrammableTransactionBuilder,
    key: &SimpleKeypair,
) -> Argument {
    let scheme = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("signature_scheme"),
        Identifier::from_static("ed25519"),
        vec![],
        vec![],
    );
    let raw_public_key = builder.pure(key.public_key().as_ref().to_vec()).unwrap();
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("public_key"),
        Identifier::from_static("create"),
        vec![],
        vec![scheme, raw_public_key],
    )
}

/// A programmable transaction that creates a `SmartAccount` with `key`
/// attached, with `proof` as the key proof.
pub fn create_account_pt(key: &SimpleKeypair, proof: Vec<u8>) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    let arguments = vec![
        public_key_argument(&mut builder, key),
        builder.pure(proof).unwrap(),
        builder.pure(VALID_UNTIL_EPOCH).unwrap(),
    ];
    let account_builder = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account_builtin_authenticator"),
        Identifier::from_static("builder_v1"),
        vec![],
        arguments,
    );
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account"),
        Identifier::from_static("build_v1"),
        vec![],
        vec![account_builder],
    );
    builder.finish()
}

/// A transaction in which `sender` creates a `SmartAccount` with `key`
/// attached, with `key`'s proof for the network `chain_identifier`.
///
/// The nonce the proof names comes from the gas payment only, so it is the
/// same with and without the proof.
pub fn create_account_tx(
    chain_identifier: &[u8; 32],
    sender: Address,
    gas: ObjectReference,
    gas_price: u64,
    key: &SimpleKeypair,
) -> Transaction {
    let tx = |proof| {
        Transaction::new_programmable(
            sender,
            vec![gas],
            create_account_pt(key, proof),
            gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            gas_price,
        )
    };
    let message = key_proof_message_v1(
        chain_identifier,
        &sender,
        true,
        &tx(vec![])
            .nonce()
            .expect("a transaction with a gas coin has a nonce"),
        VALID_UNTIL_EPOCH,
    );
    tx(sign_personal_message(key, &message))
}

/// Returns the `SmartAccount` that `effects` created: the only shared object
/// they created. Its version is the initial shared version.
pub fn created_account(effects: &TransactionEffects) -> ObjectReference {
    *effects
        .created()
        .iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .expect("the transaction creates a SmartAccount")
        .reference()
}

/// Returns the key field that `effects` created under `account`.
pub fn created_key_field(effects: &TransactionEffects, account: ObjectId) -> ObjectReference {
    let field_id = derive_dynamic_field_id(
        account,
        &TypeTag::from(PublicKeyFieldName::tag()),
        &PublicKeyFieldName::default().to_bcs_bytes(),
    )
    .unwrap();
    *effects
        .created()
        .iter()
        .find(|created| created.reference().object_id == field_id)
        .expect("the transaction attaches a key to the account")
        .reference()
}

/// Returns the reference of `object_id` after `effects` mutated it.
pub fn mutated_object(effects: &TransactionEffects, object_id: ObjectId) -> ObjectReference {
    *effects
        .mutated()
        .iter()
        .find(|mutated| mutated.reference().object_id == object_id)
        .expect("the transaction mutates the object")
        .reference()
}

/// A `MoveAuthenticator` for a transaction sent by `account`, created by
/// [`created_account`], with `call_args` as the arguments of its authenticator
/// function.
pub fn account_authenticator(account: ObjectReference, call_args: Vec<CallArg>) -> UserSignature {
    UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new(
            call_args,
            vec![],
            SharedObjectReference::new(account.object_id, account.version, false),
        )
        .into(),
    )
}

/// Executes `tx`, sent by `account` and authenticated by the built-in
/// authenticator with `key`'s signature, and returns its effects.
pub async fn execute_from_account(
    cluster: &TestCluster,
    account: ObjectReference,
    key: &SimpleKeypair,
    tx: Transaction,
) -> TransactionEffects {
    let signature: SimpleSignature = key.sign(&tx.signing_digest());
    let signature = UserSignature::Simple(signature).to_bytes();
    let authenticator = account_authenticator(
        account,
        vec![CallArg::Pure(bcs::to_bytes(&signature).unwrap())],
    );
    cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![authenticator],
        ))
        .await
}

/// Sends a transfer from `account` and returns its effects.
pub async fn send_from_account(
    cluster: &TestCluster,
    account: ObjectReference,
    key: &SimpleKeypair,
) -> TransactionEffects {
    let sender = Address::from(account.object_id);
    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let tx = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, Address::ZERO)
        .build();
    execute_from_account(cluster, account, key, tx).await
}

/// Replaces the key of `account`, `account_key`, with `new_key` through
/// `rotate_public_key`, with `new_key`'s proof, and returns the effects.
pub async fn rotate_public_key(
    cluster: &TestCluster,
    chain_identifier: &[u8; 32],
    account: ObjectReference,
    account_key: &SimpleKeypair,
    new_key: &SimpleKeypair,
) -> TransactionEffects {
    let sender = Address::from(account.object_id);
    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let tx = |proof| {
        let mut builder = ProgrammableTransactionBuilder::new();
        let arguments = vec![
            shared_account_input(&mut builder, account),
            public_key_argument(&mut builder, new_key),
            builder.pure(proof).unwrap(),
            builder.pure(VALID_UNTIL_EPOCH).unwrap(),
        ];
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::from_static("smart_account_builtin_authenticator"),
            Identifier::from_static("rotate_public_key"),
            vec![],
            arguments,
        );
        Transaction::new_programmable(
            sender,
            vec![gas],
            builder.finish(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = key_proof_message_v1(
        chain_identifier,
        &sender,
        false,
        &tx(vec![])
            .nonce()
            .expect("a transaction with a gas coin has a nonce"),
        VALID_UNTIL_EPOCH,
    );
    let tx = tx(sign_personal_message(new_key, &message));
    execute_from_account(cluster, account, account_key, tx).await
}

/// Rotates `account` from the built-in authenticator, with its key `key`, to
/// the authenticator function `module::function` of the package `package_id`,
/// which detaches the key, and returns the effects.
pub async fn rotate_to_custom_authenticator(
    cluster: &TestCluster,
    account: ObjectReference,
    key: &SimpleKeypair,
    package_id: ObjectId,
    module: &str,
    function: &str,
) -> TransactionEffects {
    let sender = Address::from(account.object_id);
    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let package_metadata = cluster
        .get_latest_object_ref(&move_package::derive_package_metadata_id(package_id))
        .await;
    let account_type: TypeTag = "0x2::smart_account::SmartAccount".parse().unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    let account_input = shared_account_input(&mut builder, account);
    let arguments = vec![
        builder
            .obj(CallArg::ImmutableOrOwned(package_metadata))
            .unwrap(),
        builder.pure(module).unwrap(),
        builder.pure(function).unwrap(),
    ];
    let authenticator = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("authenticator_function"),
        Identifier::from_static("create_auth_function_ref_v1"),
        vec![account_type],
        arguments,
    );
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account_builtin_authenticator"),
        Identifier::from_static("rotate_to_custom_authenticator_v1"),
        vec![],
        vec![account_input, authenticator],
    );
    let tx = Transaction::new_programmable(
        sender,
        vec![gas],
        builder.finish(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    execute_from_account(cluster, account, key, tx).await
}

/// Adds `account` as a mutable shared input.
pub fn shared_account_input(
    builder: &mut ProgrammableTransactionBuilder,
    account: ObjectReference,
) -> Argument {
    builder
        .obj(CallArg::Shared(SharedObjectReference::new(
            account.object_id,
            account.version,
            true,
        )))
        .unwrap()
}
