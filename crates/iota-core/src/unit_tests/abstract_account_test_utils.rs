// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A single-authority environment for abstract-account tests.
//!
//! Publishes the same Move package the end-to-end abstract-account tests use,
//! creates a shared account guarded by its ed25519 authenticator, and funds the
//! account so it can pay for its own transactions.
//!
//! Account transactions are submitted straight to consensus, as
//! `UserTransactionV1` or, with an attestation attached, `UserTransactionV2`.
//! There are no certificates on this path: consensus ordering is what
//! authorizes execution, and it is also where the transaction's shared inputs
//! get their versions assigned. Assignment therefore happens at submission, so
//! a test can rotate the account's key after building a transaction and have it
//! execute against the rotated state.

use std::{path::PathBuf, sync::Arc};

use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Encoding, Hex},
    traits::{KeyPair, Signer},
};
use iota_move_build::BuildConfig;
use iota_sdk_types::{
    Address, Argument, Identifier, MoveAuthenticatorV1, ObjectId, ObjectReference, Owner,
    ProgrammableTransaction, SharedObjectReference, TypeTag,
};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    attestation::{Attestation, AttestationData, AttestedTransaction},
    crypto::{AccountKeyPair, get_account_key_pair},
    effects::{TransactionEffects, TransactionEffectsAPI},
    messages_consensus::ConsensusTransaction,
    move_authenticator::MoveAuthenticator,
    move_package,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::UserSignature,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, Transaction, TransactionData, TransactionDataAPI,
    },
    utils::to_sender_signed_transaction,
};
use starfish_config::AuthorityIndex;

use crate::{
    authority::{
        AuthorityState, authority_test_utils::send_and_confirm_transaction,
        move_integration_tests::created_package_ref, test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
};

/// The abstract-account package shared with the end-to-end tests, relative to
/// this crate's manifest directory.
const AA_PACKAGE_PATH: &[&str] = &[
    "..",
    "iota-e2e-tests",
    "tests",
    "abstract_account",
    "abstract_account",
];
/// The module holding `AbstractAccount` and its field accessors.
const AA_MODULE: &str = "abstract_account";
const AA_ACCOUNT_TYPE: &str = "AbstractAccount";
/// The module holding `create`, `rotate_public_key` and the authenticators.
const AA_KEYED_MODULE: &str = "abstract_account_keyed";
const AA_AUTHENTICATE_ED25519: &str = "authenticate_ed25519";
pub const AA_AUTHENTICATE_ED25519_VIA_SIGNING_DIGEST: &str =
    "authenticate_ed25519_via_signing_digest";

/// Consensus accepts a validator attestation only from the block author, which
/// `SequencedConsensusTransaction::new_test` fixes at index 0.
fn attestor_index() -> AuthorityIndex {
    AuthorityIndex::new_for_test(0)
}

pub struct AbstractAccountTestEnv {
    pub authority: Arc<AuthorityState>,
    funder: Address,
    funder_key: AccountKeyPair,
    gas_ids: Vec<ObjectId>,
    package_id: ObjectId,
    metadata_ref: ObjectReference,
    account: SharedObjectReference,
    account_address: Address,
    /// Coins owned by the account. Each of its transactions takes a fresh one,
    /// so two transactions built before either executes never contend for the
    /// same owned object.
    account_gas_ids: Vec<ObjectId>,
    next_account_gas: usize,
    /// The key matching the public key the account currently stores.
    owner_key: Ed25519KeyPair,
    /// Bumped so each account transaction writes a distinct dynamic field.
    next_field_key: u8,
}

impl AbstractAccountTestEnv {
    /// Publishes the account package, creates a shared account authenticated by
    /// a fresh ed25519 key, and funds it.
    pub async fn new() -> Self {
        let (funder, funder_key) = get_account_key_pair();
        let gas_ids: Vec<ObjectId> = (0..8).map(|_| ObjectId::random()).collect();
        let objects: Vec<Object> = gas_ids
            .iter()
            .map(|id| Object::with_id_owner_for_testing(*id, funder))
            .collect();
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(&objects)
            .build()
            .await;

        let (_, owner_key) = get_account_key_pair();
        let mut env = Self {
            authority,
            funder,
            funder_key,
            gas_ids,
            package_id: ObjectId::ZERO,
            metadata_ref: ObjectReference::new(ObjectId::ZERO, 1.into(), Default::default()),
            account: SharedObjectReference {
                object_id: ObjectId::ZERO,
                initial_shared_version: 1.into(),
                mutable: false,
            },
            account_address: Address::ZERO,
            account_gas_ids: Vec::new(),
            next_account_gas: 0,
            owner_key,
            next_field_key: 0,
        };
        env.publish_package().await;
        env.create_account().await;
        env.fund_account().await;
        env
    }

    /// The account object at its current version.
    pub fn account_ref(&self) -> ObjectReference {
        self.authority
            .get_object(&self.account.object_id)
            .unwrap()
            .object_ref()
    }

    fn rgp(&self) -> u64 {
        self.authority.reference_gas_price_for_testing().unwrap()
    }

    fn budget(&self) -> u64 {
        self.rgp() * TEST_ONLY_GAS_UNIT_FOR_PUBLISH * 10
    }

    fn gas_ref(&self, index: usize) -> ObjectReference {
        self.authority
            .get_object(&self.gas_ids[index])
            .unwrap()
            .object_ref()
    }

    fn account_type(&self) -> TypeTag {
        format!("{}::{AA_MODULE}::{AA_ACCOUNT_TYPE}", self.package_id)
            .parse()
            .unwrap()
    }

    async fn publish_package(&mut self) {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.extend(AA_PACKAGE_PATH);
        let compiled_package = BuildConfig::new_for_testing().build(&path).unwrap();
        let modules = compiled_package.get_package_bytes(false);
        let dependencies = compiled_package.get_dependency_storage_package_ids();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            builder.publish_immutable(modules, dependencies);
            builder.finish()
        };
        let effects = self.execute_as_funder(pt, 0).await;
        assert!(
            effects.status().is_success(),
            "publishing the account package must succeed, got {:?}",
            effects.status()
        );

        self.package_id = created_package_ref(&effects).object_id;
        let metadata_id = move_package::derive_package_metadata_id(self.package_id);
        self.metadata_ref = self
            .authority
            .get_object(&metadata_id)
            .expect("the package carries authenticator metadata")
            .object_ref();
    }

    /// Builds an `AuthenticatorFunctionRefV1` for the ed25519 authenticator and
    /// appends it to the arguments of a call on the account package.
    fn call_with_auth_function_ref(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        target_function: &str,
        authenticate_function: &str,
        mut leading_arguments: Vec<Argument>,
    ) {
        let arguments = vec![
            builder
                .obj(CallArg::ImmutableOrOwned(self.metadata_ref))
                .unwrap(),
            builder.pure(AA_KEYED_MODULE).unwrap(),
            builder.pure(authenticate_function).unwrap(),
        ];
        let auth_fn_ref = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::from_static("authenticator_function"),
            Identifier::from_static("create_auth_function_ref_v1"),
            vec![self.account_type()],
            arguments,
        );
        leading_arguments.push(auth_fn_ref);
        builder.programmable_move_call(
            self.package_id,
            Identifier::from_static(AA_KEYED_MODULE),
            Identifier::new(target_function).unwrap(),
            vec![],
            leading_arguments,
        );
    }

    async fn create_account(&mut self) {
        let public_key = self.owner_key.public().as_ref().to_vec();
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let public_key = builder.pure(public_key).unwrap();
            self.call_with_auth_function_ref(
                &mut builder,
                "create",
                AA_AUTHENTICATE_ED25519,
                vec![public_key],
            );
            builder.finish()
        };
        let effects = self.execute_as_funder(pt, 1).await;
        assert!(
            effects.status().is_success(),
            "creating the account must succeed, got {:?}",
            effects.status()
        );

        let (account_ref, _) = effects
            .created()
            .into_iter()
            .find(|(_, owner)| matches!(owner, Owner::Shared { .. }))
            .expect("account creation must share an object");
        self.account = SharedObjectReference {
            object_id: account_ref.object_id,
            initial_shared_version: account_ref.version,
            mutable: false,
        };
        self.account_address = account_ref.object_id.into();
    }

    /// The account pays for its own transactions, so it needs coins of its own.
    async fn fund_account(&mut self) {
        for index in [2, 4, 5, 6] {
            let tx_data = TransactionData::new_transfer(
                self.account_address,
                self.gas_ref(index),
                self.funder,
                self.gas_ref(3),
                self.budget(),
                self.rgp(),
            );
            let effects = self
                .confirm(to_sender_signed_transaction(tx_data, &self.funder_key))
                .await;
            assert!(
                effects.status().is_success(),
                "funding the account must succeed, got {:?}",
                effects.status()
            );
            self.account_gas_ids.push(self.gas_ids[index]);
        }
    }

    /// Takes the next unused coin owned by the account.
    fn take_account_gas(&mut self) -> ObjectReference {
        let id = *self
            .account_gas_ids
            .get(self.next_account_gas)
            .expect("the account ran out of gas coins; fund it with more");
        self.next_account_gas += 1;
        self.authority.get_object(&id).unwrap().object_ref()
    }

    /// Runs a setup transaction that touches only owned objects, where
    /// consensus is not involved.
    async fn execute_as_funder(
        &self,
        pt: ProgrammableTransaction,
        gas_index: usize,
    ) -> TransactionEffects {
        let tx_data = TransactionData::new_programmable(
            self.funder,
            vec![self.gas_ref(gas_index)],
            pt,
            self.budget(),
            self.rgp(),
        );
        self.confirm(to_sender_signed_transaction(tx_data, &self.funder_key))
            .await
    }

    async fn confirm(&self, tx: Transaction) -> TransactionEffects {
        send_and_confirm_transaction(&self.authority, tx)
            .await
            .unwrap()
            .1
            .into_data()
    }

    /// A transaction sent by the account, authenticated with the key currently
    /// in force, so it passes against the account's present state.
    pub fn account_transaction(&mut self) -> Transaction {
        let owner_key = self.owner_key.copy();
        self.account_transaction_signed_with(&owner_key)
    }

    /// A transaction sent by the account that writes a fresh dynamic field, so
    /// the body always succeeds and only authentication decides the outcome.
    /// It is authenticated with `signing_key`, which only satisfies the
    /// authenticator while the account still stores the matching public key.
    pub fn account_transaction_signed_with(&mut self, signing_key: &Ed25519KeyPair) -> Transaction {
        let field_key = self.next_field_key;
        self.next_field_key += 1;
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let account = builder
                .obj(CallArg::Shared(SharedObjectReference {
                    mutable: true,
                    ..self.account
                }))
                .unwrap();
            let name = builder.pure(field_key).unwrap();
            let value = builder.pure(field_key).unwrap();
            builder.programmable_move_call(
                self.package_id,
                Identifier::from_static(AA_MODULE),
                Identifier::from_static("add_field"),
                vec![TypeTag::U8, TypeTag::U8],
                vec![account, name, value],
            );
            builder.finish()
        };
        self.sign_with_move_authenticator(pt, signing_key)
    }

    fn sign_with_move_authenticator(
        &mut self,
        pt: ProgrammableTransaction,
        signing_key: &Ed25519KeyPair,
    ) -> Transaction {
        let gas = self.take_account_gas();
        let tx_data = TransactionData::new_programmable(
            self.account_address,
            vec![gas],
            pt,
            self.budget(),
            self.rgp(),
        );

        // The authenticator verifies a hex-encoded raw ed25519 signature over
        // the transaction digest.
        let signature = signing_key.sign(tx_data.digest().inner());
        let signature_arg = CallArg::Pure(bcs::to_bytes(&Hex::encode(signature.as_ref())).unwrap());

        let authenticator = UserSignature::MoveAuthenticator(MoveAuthenticator::V1(
            MoveAuthenticatorV1::new_with_shared_account_object(
                vec![signature_arg],
                vec![],
                self.account,
            ),
        ));
        Transaction::from_user_sig_data(tx_data, vec![authenticator])
    }

    /// Rotates the account's public key, which supersedes the account's version
    /// and invalidates every signature made with the previous key. Returns the
    /// key that was replaced.
    pub async fn rotate_owner_key(&mut self) -> Ed25519KeyPair {
        let (_, new_key) = get_account_key_pair();
        let new_public_key = new_key.public().as_ref().to_vec();
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let account = builder
                .obj(CallArg::Shared(SharedObjectReference {
                    mutable: true,
                    ..self.account
                }))
                .unwrap();
            let public_key = builder.pure(new_public_key).unwrap();
            self.call_with_auth_function_ref(
                &mut builder,
                "rotate_public_key",
                AA_AUTHENTICATE_ED25519,
                vec![account, public_key],
            );
            builder.finish()
        };
        // Signed with the key still in force, so the rotation authenticates.
        let current_key = self.owner_key.copy();
        let tx = self.sign_with_move_authenticator(pt, &current_key);
        let effects = self.submit(tx, None).await;
        assert!(
            effects.status().is_success(),
            "rotating the owner key must succeed, got {:?}",
            effects.status()
        );

        std::mem::replace(&mut self.owner_key, new_key)
    }

    /// Rotates the account's authenticator to `authenticate_function`, keeping
    /// the current public key, so a transaction that authenticated under the
    /// previous authenticator no longer does.
    pub async fn rotate_authenticator_function(&mut self, authenticate_function: &str) {
        let public_key = self.owner_key.public().as_ref().to_vec();
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let account = builder
                .obj(CallArg::Shared(SharedObjectReference {
                    mutable: true,
                    ..self.account
                }))
                .unwrap();
            let public_key = builder.pure(public_key).unwrap();
            self.call_with_auth_function_ref(
                &mut builder,
                "rotate_public_key",
                authenticate_function,
                vec![account, public_key],
            );
            builder.finish()
        };
        let current_key = self.owner_key.copy();
        let tx = self.sign_with_move_authenticator(pt, &current_key);
        let effects = self.submit(tx, None).await;
        assert!(
            effects.status().is_success(),
            "rotating the authenticator function must succeed, got {:?}",
            effects.status()
        );
    }

    /// Runs the attestor's own dry-run, which is how a genuine attestation is
    /// produced. Only succeeds while authentication still passes.
    pub fn attest(&self, tx: &Transaction) -> Attestation {
        let epoch_store = self.authority.epoch_store_for_testing();
        let verified = epoch_store.verify_transaction(tx.clone()).unwrap();
        let (payload, _) = self
            .authority
            .attest_transaction(&verified, &epoch_store)
            .expect("attesting must succeed while authentication passes");
        Attestation::Validator {
            payload,
            attestor_index: attestor_index(),
        }
    }

    /// An attestation vouching for the transaction at the given versions, as a
    /// dishonest attestor would produce. The computation estimate is the
    /// largest one consensus accepts, so the verdict on the recorded versions
    /// is never preempted by the attested-units cap on the re-run.
    pub fn attest_with_versions(&self, object_versions: Vec<ObjectReference>) -> Attestation {
        let computation_units = self.budget() / self.rgp();
        Attestation::Validator {
            payload: AttestationData::V1 {
                computation_units,
                object_versions,
            },
            attestor_index: attestor_index(),
        }
    }

    /// Submits the transaction to consensus, with an attestation when one is
    /// given, and executes what consensus schedules. Shared input versions are
    /// assigned here, so the transaction runs against the account's state at
    /// submission time rather than at the time it was built.
    pub async fn submit(
        &self,
        tx: Transaction,
        attestation: Option<Attestation>,
    ) -> TransactionEffects {
        let consensus_tx = match attestation {
            Some(attestation) => ConsensusTransaction::new_user_transaction_v2(
                AttestedTransaction::new(tx, attestation),
            ),
            None => ConsensusTransaction::new_user_transaction_v1(tx),
        };
        let epoch_store = self.authority.epoch_store_for_testing();
        let scheduled = epoch_store
            .process_consensus_transactions_for_tests(
                vec![SequencedConsensusTransaction::new_test(consensus_tx)],
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                self.authority.get_transaction_cache_reader().as_ref(),
                &self.authority.metrics,
                true,
                &self.authority,
            )
            .await
            .unwrap();
        let executable = scheduled
            .into_iter()
            .next()
            .expect("consensus must schedule the submitted transaction");
        let (effects, _) = self
            .authority
            .try_execute_immediately(&executable, None, &epoch_store)
            .unwrap();
        effects
    }
}
