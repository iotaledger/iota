// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the validator attestation path (Phase 1).
//!
//! These tests exercise the V2 transaction submission pathway
//! (`ValidatorV2API::submit_tx`) with both `enable_white_flag_flow` and
//! `enable_validator_attestation` protocol flags enabled.  The Move abstract
//! account setup is reused from `abstract_account_tests` so the attestor
//! dry-run runs `authenticate_then_execute_transaction_to_effects`, which is
//! the more interesting branch of `attest_transaction`.
//!
//! # Protocol config propagation
//!
//! `ProtocolConfig::apply_overrides_for_testing` is thread-local and does not
//! reach validator nodes, which run in separate OS threads (see
//! `iota-swarm/src/memory/container.rs`).  We therefore rely on the
//! `IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE` / serde-env mechanism, which is
//! process-wide and readable by every thread.  See `ProtocolEnvOverride`.

use std::{net::SocketAddr, time::Duration};

use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_core::authority_client::validator_v2::ValidatorV2API;
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_types::{
    Address, Argument, Identifier, MoveAuthenticator, MoveAuthenticatorV1, ObjectId,
    ObjectReference, Owner, ProgrammableTransaction, SharedObjectReference, Transaction,
    TransactionDigest, TypeTag, UserSignature,
};
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    attestation::{AttestationTotals, AttestationVerdict},
    base_types::AuthorityName,
    effects::TransactionEffectsAPI,
    messages_checkpoint::{CheckpointSummaryExt, VerifiedCheckpoint},
    messages_grpc::TxStatusUpdate,
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use test_cluster::{TestCluster, TestClusterBuilder};

const AA_PACKAGE_PATH: &str = "tests/abstract_account/abstract_account";
const AA_MODULE_NAME: &str = "abstract_account";
const AA_ACCOUNT_NAME: &str = "AbstractAccount";
const AA_CREATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_AUTHENTICATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_AUTHENTICATE_FN_NAME_ED25519: &str = "authenticate_ed25519";

// ------------------------------------------
// --- Attestation end-to-end tests ---------
// ------------------------------------------

/// An AA transaction submitted via the V2 gRPC path with both
/// `enable_white_flag_flow` and `enable_validator_attestation` enabled must be
/// attested and accepted (status `Submitted` or `Executed`).
///
/// The MoveAuthenticator path is exercised deliberately because the attestor
/// dry-run takes the `authenticate_then_execute_transaction_to_effects` branch
/// of `attest_transaction`, which is the more interesting code path.
#[sim_test]
async fn test_aa_tx_accepted_via_v2_attestation_path() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Enable white-flag flow and validator attestation for every node.
    // Must be set BEFORE TestClusterBuilder::build() spawns node threads.
    let _env = enable_attestation_env();

    // Build a test environment and create an abstract account.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    // Fund the AA with gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Build a simple AA transaction.
    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_bytes();

    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    // Submit via the V2 attestation path.
    let results = test_env.submit_tx_v2(aa_tx).await?;

    assert!(
        !results.is_empty(),
        "Expected at least one status update from the V2 submit path"
    );
    let (_, status) = &results[0];
    assert!(
        matches!(
            status,
            TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
        ),
        "Expected Submitted or Executed from V2 path, got: {status:?}"
    );

    Ok(())
}

/// An AA transaction whose Move authentication fails (here: a wrong ed25519
/// signature) must be DROPPED by the attestor — never attested, so it can never
/// be sequenced or charged.
#[sim_test]
async fn test_aa_tx_with_failed_authentication_is_dropped() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env = enable_attestation_env();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_bytes();

    // Attach a Move authenticator that signs a tampered digest, so the in-VM
    // ed25519 verification in `authenticate_ed25519` aborts: the authentication
    // phase fails.
    let signatures = vec![test_env.create_bad_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    match test_env.submit_tx_v2(aa_tx).await {
        Ok(results) => {
            assert!(!results.is_empty(), "expected at least one status update");
            let (_, status) = &results[0];
            assert!(
                matches!(status, TxStatusUpdate::Rejected { .. }),
                "auth-failing AA tx must be rejected by the attestor, got: {status:?}"
            );
        }
        // A validator/transport-level error is also an acceptable outcome: the
        // transaction was not attested or accepted.
        Err(e) => {
            tracing::info!("auth-failing AA tx submission errored as expected: {e:?}");
        }
    }

    Ok(())
}

/// A valid AA transaction whose Move authentication needs more gas than
/// `max_auth_gas` must be refused by the attestor. The dry run meters
/// authentication against the whole budget, as execution does, so without the
/// capped pre-run the attestor would do budget-sized work on a transaction it
/// then drops unpaid.
#[sim_test]
async fn test_aa_tx_with_authentication_over_max_auth_gas_is_refused() -> Result<(), anyhow::Error>
{
    telemetry_subscribers::init_for_testing();
    // Ten gas units at the cluster's gas price, far below what the ed25519
    // verification in `authenticate_ed25519` costs.
    let _env = ProtocolEnvOverride::new(&[
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1"),
        (
            "IOTA_PROTOCOL_CONFIG_OVERRIDE_CHECKPOINT_SUMMARY_VERSION_SPECIFIC_DATA",
            "2",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_PCOOL_FLOW",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION",
            "true",
        ),
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_MAX_AUTH_GAS", "10000"),
    ]);

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    match test_env.submit_tx_v2(aa_tx).await {
        Ok(results) => {
            let (_, status) = results.first().expect("expected one status update");
            assert!(
                matches!(status, TxStatusUpdate::Rejected { .. }),
                "an authentication over max_auth_gas must be refused, got: {status:?}"
            );
        }
        // A validator/transport-level error is also an acceptable outcome: the
        // transaction was not attested or accepted.
        Err(e) => {
            tracing::info!("over-budget authentication errored as expected: {e:?}");
        }
    }

    Ok(())
}

/// An AA transaction whose Move authentication SUCCEEDS but whose transaction
/// body aborts must still be attested (status `Submitted`/`Executed`).
#[sim_test]
async fn test_aa_tx_with_body_abort_is_attested() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env = enable_attestation_env();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_double_add_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_bytes();

    // Correct signature: the authentication phase succeeds; only the body aborts.
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    let results = test_env.submit_tx_v2(aa_tx).await?;
    assert!(!results.is_empty(), "expected at least one status update");
    let (_, status) = &results[0];
    assert!(
        matches!(
            status,
            TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
        ),
        "AA tx with successful auth but a body abort must still be attested, got: {status:?}"
    );

    Ok(())
}

/// A normal (non-abstract-account) transaction whose body aborts must be
/// attested too. Here a normal owner-signed tx calls `add_field` on the shared
/// AA object; `ensure_tx_sender_is_account` aborts because the sender is the
/// owner, not the account.
#[sim_test]
async fn test_normal_tx_with_body_abort_is_attested() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env = enable_attestation_env();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;

    // Build a normal transaction (no Move authenticator) signed by the owner.
    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env
        .test_cluster
        .test_transaction_builder()
        .await
        .programmable(pt)
        .build();
    let normal_tx = test_env.test_cluster.wallet.sign_transaction(&tx_data);

    let results = test_env.submit_tx_v2(normal_tx).await?;
    assert!(!results.is_empty(), "expected at least one status update");
    let (_, status) = &results[0];
    assert!(
        matches!(
            status,
            TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
        ),
        "normal tx with a body abort must still be attested, got: {status:?}"
    );

    Ok(())
}

/// A valid verdict is recorded by the validators that judged it but not
/// certified: the summary carries only the records other than `Valid`. The
/// fullnode executes the checkpoint from state sync without the attestation,
/// so its effects must match without it, and it stores no record.
#[sim_test]
async fn valid_attestation_is_recorded_locally_but_not_certified() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env = enable_attestation_env();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let digest = *aa_tx.digest();
    test_env.submit_tx_v2(aa_tx).await?;

    let checkpoint = test_env.wait_for_transaction_checkpoint(&digest).await;
    // The quorum that certified the checkpoint judged the attestation at
    // execution; a validator still catching up may have no record yet.
    let validator_records: Vec<_> = test_env
        .test_cluster
        .swarm
        .validator_node_handles()
        .into_iter()
        .map(|handle| {
            handle.with(|node| {
                node.state()
                    .get_transaction_cache_reader()
                    .multi_get_attestation_records_in_epoch(checkpoint.epoch, &[digest])
                    .pop()
                    .expect("one result per digest")
            })
        })
        .collect();
    assert!(
        validator_records.iter().any(Option::is_some),
        "the validators that certified the checkpoint recorded a verdict"
    );
    assert!(
        validator_records
            .iter()
            .flatten()
            .all(|record| record.verdict == AttestationVerdict::Valid),
        "the validators judged the attestation valid: {validator_records:?}"
    );

    let (entries, totals, fullnode_record) =
        test_env
            .test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| {
                let state = node.state();
                let data = checkpoint
                    .parse_version_specific_data(state.epoch_store_for_testing().protocol_config())
                    .unwrap()
                    .expect("summaries carry version specific data");
                let fullnode_record = state
                    .get_transaction_cache_reader()
                    .multi_get_attestation_records_in_epoch(checkpoint.epoch, &[digest])
                    .pop()
                    .expect("one result per digest");
                (
                    data.attestation_entries().to_vec(),
                    data.attestation_totals().cloned(),
                    fullnode_record,
                )
            });
    assert!(
        entries.is_empty(),
        "a valid attestation is not certified: {entries:?}"
    );
    assert_eq!(
        totals,
        Some(AttestationTotals::default()),
        "an honest attestor leaves the totals empty"
    );
    assert_eq!(
        fullnode_record, None,
        "the fullnode stores no record for a valid attestation"
    );

    Ok(())
}

// --------------------------------------------------
// --- Protocol config env override RAII guard ------
// --------------------------------------------------

/// Enable P-COOL flow and validator attestation for every node, with the
/// checkpoint summary version that carries the attestation records. Must be
/// called BEFORE `TestClusterBuilder::build()` spawns node threads.
fn enable_attestation_env() -> ProtocolEnvOverride {
    ProtocolEnvOverride::new(&[
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1"),
        (
            "IOTA_PROTOCOL_CONFIG_OVERRIDE_CHECKPOINT_SUMMARY_VERSION_SPECIFIC_DATA",
            "2",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_PCOOL_FLOW",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION",
            "true",
        ),
    ])
}

/// Sets process-wide environment variables on construction, restores them (by
/// removing them) on drop.  Must be constructed **before**
/// `TestClusterBuilder::build()` so that validator node threads inherit the
/// values when they call `ProtocolConfig::get_for_version`.
struct ProtocolEnvOverride {
    keys: Vec<&'static str>,
}

impl ProtocolEnvOverride {
    fn new(overrides: &[(&'static str, &'static str)]) -> Self {
        for (key, val) in overrides {
            // SAFETY: set before any node thread is spawned; no concurrent env
            // readers at this point.
            unsafe { std::env::set_var(key, val) };
        }
        Self {
            keys: overrides.iter().map(|(k, _)| *k).collect(),
        }
    }
}

impl Drop for ProtocolEnvOverride {
    fn drop(&mut self) {
        for key in &self.keys {
            // SAFETY: runs at test teardown, when no node thread reads the
            // environment anymore.
            unsafe { std::env::remove_var(key) };
        }
    }
}

// --------------------------------------------------
// --- Minimal test environment ---------------------
// --------------------------------------------------

struct TestEnvironment {
    test_cluster: TestCluster,
    owner: Option<Address>,
    aa_package_id: Option<ObjectId>,
    aa_package_metadata_ref: Option<ObjectReference>,
    aa_ref: Option<ObjectReference>,
}

impl TestEnvironment {
    async fn new() -> Self {
        let test_cluster = TestClusterBuilder::new().build().await;
        Self {
            test_cluster,
            owner: None,
            aa_package_id: None,
            aa_package_metadata_ref: None,
            aa_ref: None,
        }
    }

    async fn setup_abstract_account(
        &mut self,
        authenticate_fn_name: &str,
    ) -> Result<(), anyhow::Error> {
        self.owner = Some(
            self.test_cluster
                .wallet
                .config()
                .keystore()
                .addresses()
                .first()
                .cloned()
                .unwrap(),
        );

        let path = [env!("CARGO_MANIFEST_DIR"), AA_PACKAGE_PATH]
            .iter()
            .collect();
        let aa_package_id = publish_package(self.test_cluster.wallet(), path)
            .await
            .object_id;
        let aa_package_metadata_id = move_package::derive_package_metadata_id(aa_package_id);
        let aa_package_metadata_ref = self
            .test_cluster
            .get_latest_object_ref(&aa_package_metadata_id)
            .await;

        self.aa_package_id = Some(aa_package_id);
        self.aa_package_metadata_ref = Some(aa_package_metadata_ref);

        let transaction = self
            .craft_create_abstract_account(authenticate_fn_name)
            .await?;

        let response = self.test_cluster.execute_transaction(transaction).await;

        self.aa_ref = response.created().into_iter().find_map(|created| {
            matches!(created.owner(), Owner::Shared { .. }).then_some(*created.reference())
        });

        assert!(
            self.aa_ref.is_some(),
            "Abstract account creation did not produce a shared object"
        );
        Ok(())
    }

    async fn craft_create_abstract_account(
        &self,
        authenticate_fn_name: &str,
    ) -> anyhow::Result<TransactionEnvelope> {
        let (Some(owner), Some(aa_package_id), Some(aa_package_metadata_ref)) =
            (self.owner, self.aa_package_id, self.aa_package_metadata_ref)
        else {
            anyhow::bail!("setup_abstract_account must be called first");
        };

        let aa_owner_pk = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .get_key(&owner)?
            .public();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let arguments = vec![
                builder.obj(CallArg::ImmutableOrOwned(aa_package_metadata_ref))?,
                builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
            ];
            if let Argument::Result(auth_fn_ref) = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                Identifier::from_static("authenticator_function"),
                Identifier::from_static("create_auth_function_ref_v1"),
                vec![abstract_account_type_tag(&aa_package_id)],
                arguments,
            ) {
                let arguments = vec![
                    builder.pure(aa_owner_pk.as_ref())?,
                    Argument::Result(auth_fn_ref),
                ];
                builder.programmable_move_call(
                    aa_package_id,
                    Identifier::from_static(AA_CREATE_MODULE_NAME),
                    Identifier::from_static("create"),
                    vec![],
                    arguments,
                );
            }
            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(pt)
            .build();

        Ok(self.test_cluster.wallet.sign_transaction(&tx_data))
    }

    fn craft_aa_simple_ptb(&self) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not set up yet");
        };
        let mut builder = ProgrammableTransactionBuilder::new();
        let arguments = vec![
            builder.obj(CallArg::Shared(SharedObjectReference {
                object_id: aa_ref.object_id,
                initial_shared_version: aa_ref.version,
                mutable: true,
            }))?,
            builder.pure(1_u8)?,
            builder.pure(2_u8)?,
        ];
        builder.programmable_move_call(
            aa_package_id,
            Identifier::from_static(AA_MODULE_NAME),
            Identifier::from_static("add_field"),
            vec![TypeTag::U8, TypeTag::U8],
            arguments,
        );
        Ok(builder.finish())
    }

    /// A PTB that adds the same dynamic field key twice, so the second
    /// `add_field` aborts in the transaction body (the auth phase, if present,
    /// is unaffected).
    fn craft_aa_double_add_ptb(&self) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not set up yet");
        };
        let mut builder = ProgrammableTransactionBuilder::new();
        let aa_arg = builder.obj(CallArg::Shared(SharedObjectReference {
            object_id: aa_ref.object_id,
            initial_shared_version: aa_ref.version,
            mutable: true,
        }))?;
        let type_args = vec![TypeTag::U8, TypeTag::U8];
        for value in [2_u8, 3_u8] {
            // Same key (1u8) both times; the second `dynamic_field::add` aborts.
            let key = builder.pure(1_u8)?;
            let val = builder.pure(value)?;
            builder.programmable_move_call(
                aa_package_id,
                Identifier::from_static(AA_MODULE_NAME),
                Identifier::from_static("add_field"),
                type_args.clone(),
                vec![aa_arg, key, val],
            );
        }
        Ok(builder.finish())
    }

    async fn craft_tx_from_pt(
        &self,
        pt: ProgrammableTransaction,
        gas_coin: ObjectReference,
        sender: Address,
    ) -> anyhow::Result<Transaction> {
        let gas_price = self.test_cluster.get_reference_gas_price().await;
        Ok(Transaction::new_programmable_allow_sponsor(
            sender,
            vec![gas_coin],
            pt,
            gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            gas_price,
            sender,
        ))
    }

    fn create_move_authenticator_for_ed25519(
        &self,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<UserSignature> {
        let Some(owner) = self.owner else {
            anyhow::bail!("Abstract account not set up yet");
        };

        self.create_move_authenticator_signed_by(&owner, tx_digest)
    }

    /// Creates a Move authenticator for the account that carries the signature
    /// of the key of `signer` over `tx_digest`.
    fn create_move_authenticator_signed_by(
        &self,
        signer: &Address,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<UserSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not set up yet");
        };
        let signature = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_hashed(signer, tx_digest)?;

        let hex_encoded_signature: String = Hex::encode(signature.to_bytes())
            .chars()
            .skip(2)
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
        let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);

        Ok(UserSignature::MoveAuthenticator(MoveAuthenticator::V1(
            MoveAuthenticatorV1::new(
                vec![signature_call_arg],
                vec![],
                SharedObjectReference {
                    object_id: aa_ref.object_id,
                    initial_shared_version: aa_ref.version,
                    mutable: false,
                },
            ),
        )))
    }

    /// Like [`Self::create_move_authenticator_for_ed25519`], but signs a
    /// *tampered* digest so the in-VM ed25519 verification fails — the
    /// authentication phase aborts.
    fn create_bad_move_authenticator_for_ed25519(
        &self,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<UserSignature> {
        let mut tampered = *tx_digest;
        tampered[0] ^= 0xff;
        self.create_move_authenticator_for_ed25519(&tampered)
    }

    /// Waits until the fullnode has executed the checkpoint that contains the
    /// transaction, and returns it.
    async fn wait_for_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> VerifiedCheckpoint {
        let state = self.test_cluster.fullnode_handle.iota_node.state();
        let (sequence_number, _) = state
            .wait_for_checkpoint_inclusion(&[*digest], Duration::from_secs(60))
            .await
            .expect("the fullnode must execute the checkpoint with the transaction")
            .remove(digest)
            .expect("the transaction was waited for");
        state
            .get_checkpoint_by_sequence_number(sequence_number)
            .expect("db error")
            .expect("an executed checkpoint is stored")
    }

    /// Submit a transaction via the V2 gRPC path on the first available
    /// validator.
    async fn submit_tx_v2(
        &self,
        tx: TransactionEnvelope,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, iota_types::error::IotaError> {
        let validator = *self
            .test_cluster
            .get_validator_pubkeys()
            .iter()
            .min()
            .expect("the cluster has validators");

        self.submit_tx_v2_to(tx, &validator).await
    }

    /// Submit a transaction via the V2 gRPC path on `validator`.
    async fn submit_tx_v2_to(
        &self,
        tx: TransactionEnvelope,
        validator: &AuthorityName,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, iota_types::error::IotaError> {
        let client = self.test_cluster.fullnode_handle.iota_node.with(|node| {
            let orchestrator = node
                .transaction_orchestrator()
                .expect("TransactionOrchestrator not initialised on fullnode");

            let agg = orchestrator.clone_authority_aggregator();

            agg.authority_clients
                .get(validator)
                .expect("the validator is in the committee")
                .authority_client()
                .clone()
        });

        client
            .submit_tx(vec![tx], Some(SocketAddr::new([127, 0, 0, 1].into(), 0)))
            .await
    }
}

fn abstract_account_type_tag(aa_package_id: &ObjectId) -> TypeTag {
    use std::str::FromStr;
    TypeTag::from_str(&format!(
        "{aa_package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}"
    ))
    .unwrap()
}

// ------------------------------------------------------------------
// --- A faulty attestor, judged alike by every validator -----------
// ------------------------------------------------------------------

/// One validator attests through two fail points in
/// `AuthorityState::attest_transaction()`: one makes it attest a transaction
/// whose Move authentication fails in its dry run, the other replaces the
/// object versions it records. Every validator must judge such an attestation
/// alike, whatever earlier epochs of object versions it keeps: the same
/// verdicts and with them the same effects, the same certified records and
/// totals, and the same penalty at the epoch end.
///
/// Fail points exist only under the simulator or the `fail_points` cfg, and
/// simulator node ids only under the simulator, where every node runs on the
/// test thread, so [`ProtocolConfig::apply_overrides_for_testing()`] reaches
/// them all.
///
/// [`ProtocolConfig::apply_overrides_for_testing()`]: iota_protocol_config::ProtocolConfig::apply_overrides_for_testing
#[cfg(msim)]
mod faulty_attestor {
    use std::collections::{BTreeMap, BTreeSet};

    use iota_macros::{clear_fail_point, register_fail_point_arg};
    use iota_node::IotaNodeHandle;
    use iota_protocol_config::{OverrideGuard, ProtocolConfig};
    use iota_sdk_types::{
        EndOfEpochTransactionKind, ExecutionError, ExecutionStatus, GasCostSummary,
        SignatureScheme, TransactionEffects, TransactionKind,
    };
    use iota_simulator::task::NodeId;
    use iota_types::{
        attestation::AttestationRecord,
        base_types::{ConciseableName, EpochId},
        crypto::PublicKey,
        iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait,
        messages_checkpoint::CheckpointSequenceNumber,
        object::Object,
        storage::ObjectKey,
    };
    use tokio::time::{sleep, timeout};

    use super::*;

    const NUM_VALIDATORS: usize = 4;
    /// The validator that attests through the fail points, by its position in
    /// the network config.
    const ATTESTOR: usize = 0;
    /// The validator the honest transactions go to.
    const HONEST: usize = 1;
    /// The validator that keeps the superseded object versions of
    /// [`RETAINED_EPOCHS`] earlier epochs; the others keep only the current
    /// epoch's.
    const RETAINING: usize = 3;
    const RETAINED_EPOCHS: u64 = 5;

    /// An epoch duration longer than the test runs, so that only the test ends
    /// an epoch.
    const EPOCH_DURATION_MS: u64 = 3_600_000;

    /// The balance of each gas coin the faucet sends the account.
    const GAS_COIN_BALANCE: u64 = 20_000_000_000;
    /// The amount the body of an account transaction moves from its gas coin
    /// to the account.
    const BODY_TRANSFER_AMOUNT: u64 = 1;

    /// The protocol config's `attestation_max_refuted_units_squared`: the sum
    /// of the squared units an attestor's refuted attestations wasted at which
    /// the cut takes its whole score, and from which its attested transactions
    /// are dropped for the rest of the epoch. A refuted attestation of the test
    /// wastes one `gas_rounding_step` of 1,000 units in execution's run, and
    /// the one the judge re-runs another in the re-run, so the sum stays far
    /// below it and the cut is a small part of the score.
    const MAX_REFUTED_UNITS_SQUARED: u64 = 50_000u64.pow(2);

    /// The score every validator gets while the protocol feature flag
    /// `pass_calculated_validator_scores_to_advance_epoch` is off: a copy of
    /// `MAX_SCORE` in `crates/iota-core/src/authority/scorer.rs`, which is
    /// crate-private.
    const MAX_SCORE: u64 = u16::MAX as u64 + 1;

    /// Overrides the protocol config of every node until the returned guard is
    /// dropped. The overrides turn on the protocol feature flag
    /// `enable_validator_attestation`, with the
    /// `checkpoint_summary_version_specific_data` of 2 that the flag requires,
    /// set `attestation_max_refuted_units_squared` to
    /// [`MAX_REFUTED_UNITS_SQUARED`], and turn off the protocol feature flag
    /// `pass_calculated_validator_scores_to_advance_epoch`, so that every
    /// score is [`MAX_SCORE`] before the penalty.
    fn enable_attestation_with_fault_penalty() -> OverrideGuard {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_validator_attestation_for_testing(true);
            config.set_checkpoint_summary_version_specific_data_for_testing(2);
            config.set_attestation_max_refuted_units_squared_for_testing(MAX_REFUTED_UNITS_SQUARED);
            config.set_pass_calculated_validator_scores_to_advance_epoch_for_testing(false);

            config
        })
    }

    /// The verdict every node must reach on a scenario's attestation, with who
    /// pays for the transaction and what the record carries.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ExpectedVerdict {
        /// The attestation is valid: the body ran, or the judge's re-run at the
        /// recorded versions passed. The gas owner pays, and no record is
        /// certified.
        Valid,
        /// The attestation is refuted without a re-run: the failure reproduces
        /// at the recorded versions, or a recorded version refutes the
        /// attestation on its own. Nobody pays, and the record carries the
        /// units of execution's own run.
        Refuted,
        /// The attestation is refuted by the judge's re-run, which fails at the
        /// recorded versions too. Nobody pays, and the record carries the units
        /// of execution's own run and of the re-run.
        RefutedByRerun,
    }

    struct Scenario {
        name: &'static str,
        digest: TransactionDigest,
        attestor: AuthorityName,
        /// Whether Move authentication passes at execution.
        authenticates: bool,
        expected_verdict: ExpectedVerdict,
    }

    /// Runs an honest attestation and five attestations of the faulty attestor
    /// in one epoch. Move authentication of the five fails at execution, and
    /// the account version the attestor records decides the verdict:
    /// - the version the attestor read: [`Refuted`], no re-run;
    /// - a version superseded in the previous epoch: [`Refuted`], no re-run;
    /// - a version ahead of the executed one: [`Refuted`], no re-run;
    /// - a version superseded this epoch, holding the signer's key: [`Valid`];
    /// - the same version, with a wrong signature: [`Refuted`] by the re-run.
    ///
    /// Only the validator that keeps earlier epochs still holds the version
    /// superseded in the previous epoch. The judge re-runs authentication only
    /// at versions superseded this epoch, so that validator refutes like the
    /// others.
    ///
    /// Every node must execute each transaction with the same effects, which
    /// charge the gas owner for a valid attestation and nobody for a refuted
    /// one. Every validator must build the checkpoints as certified and record
    /// the verdict the checkpoint certifies, or [`Valid`] where it certifies
    /// none. The fullnode must copy the certified records. The refutation by
    /// the re-run must carry more units than the refutations without one. The
    /// change epoch transaction must cut the attestor's score by the certified
    /// totals and leave the other scores whole.
    ///
    /// [`Valid`]: AttestationVerdict::Valid
    /// [`Refuted`]: AttestationVerdict::Refuted
    #[sim_test]
    async fn every_validator_reaches_the_same_verdicts_totals_and_penalty()
    -> Result<(), anyhow::Error> {
        telemetry_subscribers::init_for_testing();
        let _config = enable_attestation_with_fault_penalty();

        let mut env = TestEnvironment::with_a_retaining_validator().await;
        env.setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
            .await?;
        let attestor = env.validator_at(ATTESTOR);
        let honest = env.validator_at(HONEST);
        let attestor_sim_id = env
            .node(&attestor)
            .with(|_| iota_simulator::current_simnode_id());

        // Epoch 0: rotate the account's key once, so that the version the
        // rotation supersedes belongs to an earlier epoch than the scenarios.
        let first_key = env.owner.expect("the account is set up");
        let account_with_first_key = env.account_ref().await;
        let second_key = env.rotate_owner_key().await?;

        env.test_cluster.force_new_epoch().await;
        assert_eq!(
            read_change_epoch_scores(&env.test_cluster, 0),
            vec![MAX_SCORE; NUM_VALIDATORS],
            "without a refuted attestation, every score stays whole"
        );
        env.wait_until_only_the_retaining_validator_holds(0, &account_with_first_key)
            .await;

        // Epoch 1.
        let epoch = 1;
        let mut scenarios = Vec::new();

        let tx = env.craft_account_transaction_signed_by(&second_key).await?;
        scenarios.push(Scenario {
            name: "honest attestation",
            digest: env.submit_to(tx, &honest).await,
            attestor: honest,
            authenticates: true,
            expected_verdict: ExpectedVerdict::Valid,
        });

        let tx = env
            .craft_account_transaction_signed_over_another_digest_by(&second_key)
            .await?;
        scenarios.push(Scenario {
            name: "fails at the versions the attestor read",
            digest: env
                .submit_to_faulty_attestor(tx, &attestor, attestor_sim_id, None)
                .await,
            attestor,
            authenticates: false,
            expected_verdict: ExpectedVerdict::Refuted,
        });

        let tx = env.craft_account_transaction_signed_by(&first_key).await?;
        scenarios.push(Scenario {
            name: "recorded version superseded in the previous epoch",
            digest: env
                .submit_to_faulty_attestor(
                    tx,
                    &attestor,
                    attestor_sim_id,
                    Some(vec![account_with_first_key]),
                )
                .await,
            attestor,
            authenticates: false,
            expected_verdict: ExpectedVerdict::Refuted,
        });

        let account_with_second_key = env.account_ref().await;
        let version_ahead = ObjectReference::new(
            account_with_second_key.object_id,
            account_with_second_key.version + 1,
            account_with_second_key.digest,
        );
        let tx = env
            .craft_account_transaction_signed_over_another_digest_by(&second_key)
            .await?;
        scenarios.push(Scenario {
            name: "recorded version ahead of the executed one",
            digest: env
                .submit_to_faulty_attestor(
                    tx,
                    &attestor,
                    attestor_sim_id,
                    Some(vec![version_ahead]),
                )
                .await,
            attestor,
            authenticates: false,
            expected_verdict: ExpectedVerdict::Refuted,
        });

        // The scenarios so far must execute at the account version that the
        // next rotation supersedes.
        for scenario in &scenarios {
            env.wait_for_transaction_checkpoint(&scenario.digest).await;
        }
        // Rotate the key again, so that the version holding the second key is
        // superseded in this epoch.
        env.rotate_owner_key().await?;

        let tx = env.craft_account_transaction_signed_by(&second_key).await?;
        scenarios.push(Scenario {
            name: "recorded version superseded this epoch",
            digest: env
                .submit_to_faulty_attestor(
                    tx,
                    &attestor,
                    attestor_sim_id,
                    Some(vec![account_with_second_key]),
                )
                .await,
            attestor,
            authenticates: false,
            expected_verdict: ExpectedVerdict::Valid,
        });

        let tx = env
            .craft_account_transaction_signed_over_another_digest_by(&second_key)
            .await?;
        scenarios.push(Scenario {
            name: "fails at a recorded version superseded this epoch",
            digest: env
                .submit_to_faulty_attestor(
                    tx,
                    &attestor,
                    attestor_sim_id,
                    Some(vec![account_with_second_key]),
                )
                .await,
            attestor,
            authenticates: false,
            expected_verdict: ExpectedVerdict::RefutedByRerun,
        });

        for scenario in &scenarios {
            env.wait_for_transaction_checkpoint(&scenario.digest).await;
            let effects = wait_for_effects_on_every_node(&env.test_cluster, scenario.digest).await;
            assert!(
                effects.iter().all(|on_node| *on_node == effects[0]),
                "{}: every node must reach the same effects",
                scenario.name
            );
            assert_status(scenario, &effects[0]);
            let gas_coin = env
                .test_cluster
                .get_object_from_fullnode_store(effects[0].gas_object().reference().object_id())
                .await
                .expect("the fullnode holds the gas coin");
            assert_payment(scenario, &effects[0], &gas_coin);
        }

        // A different verdict or total on a validator would change the
        // summary it builds. Validators drop their own summaries of earlier
        // epochs, so this runs in the epoch of the scenarios.
        assert_validators_build_the_certified_checkpoints(&env.test_cluster).await;

        let fullnode = &env.test_cluster.fullnode_handle.iota_node;
        let (certified, _) = read_certified_attestations(
            &env.test_cluster,
            epoch,
            highest_executed_checkpoint(fullnode),
        );
        let refuted: BTreeSet<_> = scenarios
            .iter()
            .filter(|scenario| scenario.expected_verdict != ExpectedVerdict::Valid)
            .map(|scenario| scenario.digest)
            .collect();
        assert_eq!(
            certified.keys().copied().collect::<BTreeSet<_>>(),
            refuted,
            "exactly the refuted attestations are certified"
        );
        for scenario in &scenarios {
            env.assert_records(scenario, certified.get(&scenario.digest).copied());
        }
        // Execution's own run counts whether or not the judge re-runs, so the
        // refutation by the re-run carries more units than the others.
        let refuted_units = |expected_verdict| {
            scenarios
                .iter()
                .filter(|scenario| scenario.expected_verdict == expected_verdict)
                .map(|scenario| {
                    let AttestationVerdict::Refuted { units } = certified[&scenario.digest].verdict
                    else {
                        panic!("{}: the record is refuted", scenario.name)
                    };

                    units
                })
                .collect::<Vec<_>>()
        };
        let most_without_rerun = refuted_units(ExpectedVerdict::Refuted)
            .into_iter()
            .max()
            .expect("scenarios are refuted without a re-run");
        for units in refuted_units(ExpectedVerdict::RefutedByRerun) {
            assert!(
                units > most_without_rerun,
                "the refutation by the re-run must add the re-run's units to execution's: \
                 {units} vs {most_without_rerun}"
            );
        }

        let attestor_index = env.authority_index(&attestor);
        let attestor_position = env.position_in_committee_validators(&attestor);

        env.test_cluster.force_new_epoch().await;
        let last = fullnode
            .with(|node| {
                node.state()
                    .get_checkpoint_store()
                    .get_epoch_last_checkpoint_seq_number(epoch)
            })
            .unwrap()
            .expect("the epoch ended");
        let (certified_through_the_end, totals) =
            read_certified_attestations(&env.test_cluster, epoch, last);
        assert_eq!(
            certified_through_the_end, certified,
            "no attestation is certified after the scenarios"
        );
        assert_eq!(
            totals
                .refuted_units_squared
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![attestor_index],
            "only the faulty attestor has refuted attestations"
        );
        // A sum of `MAX_REFUTED_UNITS_SQUARED` takes the whole score, a smaller
        // one its fraction.
        let full_cut = u128::from(MAX_REFUTED_UNITS_SQUARED);
        let wasted = u128::from(totals.refuted_units_squared[&attestor_index]);
        let cut = u64::try_from(u128::from(MAX_SCORE) * wasted.min(full_cut) / full_cut).unwrap();
        assert!(
            cut > 0 && cut < MAX_SCORE,
            "the refuted attestations must cut part of the attestor's score, not {cut}"
        );
        let mut expected_scores = vec![MAX_SCORE; NUM_VALIDATORS];
        expected_scores[attestor_position] -= cut;
        assert_eq!(
            read_change_epoch_scores(&env.test_cluster, epoch),
            expected_scores,
            "the change epoch transaction cuts the attestor's score alone"
        );

        for handle in env.test_cluster.all_node_handles() {
            assert_eq!(
                read_verdict_mismatches(&handle),
                0.0,
                "no node may record a verdict that differs from the certified one"
            );
        }

        Ok(())
    }

    /// Helpers of the test. The ones that use the account need it set up first
    /// with [`Self::setup_abstract_account()`].
    impl TestEnvironment {
        /// Builds an environment on [`NUM_VALIDATORS`] validators, of which the
        /// one at [`RETAINING`] keeps the superseded object versions of
        /// [`RETAINED_EPOCHS`] earlier epochs and the others keep only the
        /// current epoch's. Panics if a validator's config keeps another
        /// number of earlier epochs.
        async fn with_a_retaining_validator() -> Self {
            let test_cluster = TestClusterBuilder::new()
                .with_num_validators(NUM_VALIDATORS)
                .with_epoch_duration_ms(EPOCH_DURATION_MS)
                .with_node_config_overrides(vec![
                    format!(
                        "validator-{RETAINING}:authority-store-pruning-config.num-epochs-to-retain={RETAINED_EPOCHS}"
                    )
                    .parse()
                    .unwrap(),
                ])
                .build()
                .await;
            let env = Self {
                test_cluster,
                owner: None,
                aa_package_id: None,
                aa_package_metadata_ref: None,
                aa_ref: None,
            };
            for position in 0..NUM_VALIDATORS {
                let retained = env
                    .test_cluster
                    .swarm
                    .node(&env.validator_at(position))
                    .unwrap()
                    .config()
                    .authority_store_pruning_config
                    .num_epochs_to_retain;
                let expected = if position == RETAINING {
                    RETAINED_EPOCHS
                } else {
                    0
                };
                assert_eq!(
                    retained, expected,
                    "validator {position} keeps {expected} earlier epochs"
                );
            }

            env
        }

        /// The validator at `position` in
        /// [`NetworkConfig::validator_configs()`], the position that a
        /// `validator-<N>` node config override names.
        ///
        /// [`NetworkConfig::validator_configs()`]: iota_swarm_config::network_config::NetworkConfig::validator_configs
        fn validator_at(&self, position: usize) -> AuthorityName {
            self.test_cluster.swarm.config().validator_configs()[position].authority_public_key()
        }

        /// The [`Committee::authority_index()`] of validator `name`, the index
        /// that attestation records and totals name an attestor by.
        ///
        /// [`Committee::authority_index()`]: iota_types::committee::Committee::authority_index
        fn authority_index(&self, name: &AuthorityName) -> u8 {
            let index = self.test_cluster.fullnode_handle.iota_node.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .committee()
                    .authority_index(name)
                    .expect("the validator is in the committee")
            });

            u8::try_from(index).expect("committee indices fit a u8")
        }

        /// The position of validator `name` in the epoch's
        /// [`committee_validators()`], the order Move reads the scores in.
        ///
        /// [`committee_validators()`]: EpochStartSystemStateTrait::committee_validators
        fn position_in_committee_validators(&self, name: &AuthorityName) -> usize {
            self.test_cluster.fullnode_handle.iota_node.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .epoch_start_state()
                    .committee_validators()
                    .iter()
                    .position(|validator| validator.authority_name() == *name)
                    .expect("the validator is a committee member")
            })
        }

        fn node(&self, name: &AuthorityName) -> IotaNodeHandle {
            self.test_cluster
                .swarm
                .node(name)
                .unwrap()
                .get_node_handle()
                .unwrap()
        }

        fn account_address(&self) -> anyhow::Result<Address> {
            let Some(aa_ref) = self.aa_ref else {
                anyhow::bail!("Abstract account not set up yet");
            };

            Ok(aa_ref.object_id.into())
        }

        /// The reference of the account's latest version, as the fullnode
        /// holds it.
        async fn account_ref(&self) -> ObjectReference {
            let aa_ref = self.aa_ref.expect("the account is set up");

            self.test_cluster
                .get_latest_object_ref(&aa_ref.object_id)
                .await
        }

        /// Waits until the validator at [`RETAINING`] is the only validator
        /// that still holds `object`. `object` is a version superseded in
        /// `epoch`, an earlier epoch than the current one. Panics if another
        /// validator still holds it after 60 seconds, or if the validator at
        /// [`RETAINING`] does not hold it.
        async fn wait_until_only_the_retaining_validator_holds(
            &self,
            epoch: EpochId,
            object: &ObjectReference,
        ) {
            let holds = |position: usize| {
                self.node(&self.validator_at(position)).with(|node| {
                    node.state()
                        .get_historic_objects()
                        .get_in_epoch(epoch, &ObjectKey(object.object_id, object.version))
                        .unwrap()
                        .is_some()
                })
            };
            timeout(Duration::from_secs(60), async {
                while (0..NUM_VALIDATORS).any(|position| position != RETAINING && holds(position)) {
                    sleep(Duration::from_millis(500)).await;
                }
            })
            .await
            .expect("the validators that keep only the current epoch drop the earlier ones");
            assert!(
                holds(RETAINING),
                "the validator that keeps earlier epochs still holds the version"
            );
        }

        /// Rotates the account to a new key through a transaction the account
        /// authenticates with its current key, and makes the new key the
        /// owner. Returns the new key's address.
        async fn rotate_owner_key(&mut self) -> anyhow::Result<Address> {
            let new_owner = self
                .test_cluster
                .wallet
                .config_mut()
                .keystore_mut()
                .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)?
                .0;
            let new_public_key = self
                .test_cluster
                .wallet
                .config()
                .keystore()
                .get_key(&new_owner)?
                .public();
            let Some(owner) = self.owner else {
                anyhow::bail!("Abstract account not set up yet");
            };
            let account = self.account_address()?;
            let gas = self.fund_account().await?;
            let tx = self
                .craft_tx_from_pt(
                    self.craft_rotate_owner_key_ptb(&new_public_key)?,
                    gas,
                    account,
                )
                .await?;
            let signature =
                self.create_move_authenticator_signed_by(&owner, &tx.digest().into_bytes())?;
            // Panics unless the rotation succeeds.
            self.test_cluster
                .execute_transaction(TransactionEnvelope::from_user_sig_data(tx, vec![signature]))
                .await;
            self.owner = Some(new_owner);

            Ok(new_owner)
        }

        /// The body of a transaction that rotates the account to `public_key`,
        /// with the Ed25519 authenticator.
        fn craft_rotate_owner_key_ptb(
            &self,
            public_key: &PublicKey,
        ) -> anyhow::Result<ProgrammableTransaction> {
            let (Some(aa_ref), Some(aa_package_id), Some(aa_package_metadata_ref)) = (
                self.aa_ref,
                self.aa_package_id,
                self.aa_package_metadata_ref,
            ) else {
                anyhow::bail!("Abstract account not set up yet");
            };
            let mut builder = ProgrammableTransactionBuilder::new();
            let arguments = vec![
                builder.obj(CallArg::ImmutableOrOwned(aa_package_metadata_ref))?,
                builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(AA_AUTHENTICATE_FN_NAME_ED25519)?,
            ];
            let Argument::Result(auth_fn_ref) = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                Identifier::from_static("authenticator_function"),
                Identifier::from_static("create_auth_function_ref_v1"),
                vec![abstract_account_type_tag(&aa_package_id)],
                arguments,
            ) else {
                anyhow::bail!("a Move call returns a result");
            };
            let arguments = vec![
                builder.obj(CallArg::Shared(SharedObjectReference {
                    object_id: aa_ref.object_id,
                    initial_shared_version: aa_ref.version,
                    mutable: true,
                }))?,
                builder.pure(public_key.as_ref())?,
                Argument::Result(auth_fn_ref),
            ];
            builder.programmable_move_call(
                aa_package_id,
                Identifier::from_static(AA_CREATE_MODULE_NAME),
                Identifier::from_static("rotate_public_key"),
                vec![],
                arguments,
            );

            Ok(builder.finish())
        }

        /// Sends the account a new gas coin of [`GAS_COIN_BALANCE`] from the
        /// faucet and returns the coin's reference.
        async fn fund_account(&self) -> anyhow::Result<ObjectReference> {
            let account = self.account_address()?;
            let rgp = self.test_cluster.get_reference_gas_price().await;

            Ok(self
                .test_cluster
                .fund_address_and_return_gas(rgp, Some(GAS_COIN_BALANCE), account)
                .await)
        }

        /// Crafts a transaction of the account whose body only moves
        /// [`BODY_TRANSFER_AMOUNT`] from its gas coin, a new coin from
        /// [`Self::fund_account()`], to the account. The body leaves the
        /// account object alone, so only the key rotations move the account's
        /// version.
        async fn craft_account_transaction(&self) -> anyhow::Result<Transaction> {
            let account = self.account_address()?;
            let gas = self.fund_account().await?;
            let mut builder = ProgrammableTransactionBuilder::new();
            builder.transfer_iota(account, Some(BODY_TRANSFER_AMOUNT));

            self.craft_tx_from_pt(builder.finish(), gas, account).await
        }

        /// Crafts a transaction as [`Self::craft_account_transaction()`] does,
        /// authenticated by a Move authenticator that carries the signature of
        /// the key of `signer` over the transaction's digest.
        async fn craft_account_transaction_signed_by(
            &self,
            signer: &Address,
        ) -> anyhow::Result<TransactionEnvelope> {
            let tx = self.craft_account_transaction().await?;
            let signature =
                self.create_move_authenticator_signed_by(signer, &tx.digest().into_bytes())?;

            Ok(TransactionEnvelope::from_user_sig_data(tx, vec![signature]))
        }

        /// Crafts a transaction as [`Self::craft_account_transaction()`] does,
        /// authenticated by a Move authenticator that carries the signature of
        /// the key of `signer` over another digest, so that the authenticator
        /// rejects the transaction whatever key the account holds.
        async fn craft_account_transaction_signed_over_another_digest_by(
            &self,
            signer: &Address,
        ) -> anyhow::Result<TransactionEnvelope> {
            let tx = self.craft_account_transaction().await?;
            let mut other_digest = tx.digest().into_bytes();
            other_digest[0] ^= 0xff;
            let signature = self.create_move_authenticator_signed_by(signer, &other_digest)?;

            Ok(TransactionEnvelope::from_user_sig_data(tx, vec![signature]))
        }

        /// Submits `tx` to `validator` and returns the digest of `tx` once the
        /// validator has submitted it to consensus. Panics if the validator
        /// does not.
        async fn submit_to(
            &self,
            tx: TransactionEnvelope,
            validator: &AuthorityName,
        ) -> TransactionDigest {
            let digest = *tx.digest();
            let results = self
                .submit_tx_v2_to(tx, validator)
                .await
                .expect("the validator answers the submission");
            assert!(
                matches!(
                    results.as_slice(),
                    [(
                        _,
                        TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
                    )]
                ),
                "the validator must attest and submit the transaction, got {results:?}"
            );

            digest
        }

        /// Submits `tx` to the faulty attestor and returns the digest of `tx`
        /// once the attestor has submitted it to consensus. `attestor` names
        /// the faulty attestor and `attestor_sim_id` is its simulator node id.
        /// The attestor attests `tx` although the Move authentication of `tx`
        /// fails there, and claims every unit the budget pays for, so that the
        /// claim never cuts the judge's re-run short. When `recorded_versions`
        /// is given, the attestor records them in place of the versions its dry
        /// run read. `recorded_versions` need not be a whole record, which also
        /// holds the account's fields the authenticator read: the judge
        /// compares only the versions a record names, and its re-run resolves
        /// the fields under the recorded account version, so the account's
        /// version alone decides the verdict.
        async fn submit_to_faulty_attestor(
            &self,
            tx: TransactionEnvelope,
            attestor: &AuthorityName,
            attestor_sim_id: NodeId,
            recorded_versions: Option<Vec<ObjectReference>>,
        ) -> TransactionDigest {
            let transaction = tx.data().transaction();
            let claimed_units = transaction.gas_budget() / transaction.gas_price();
            register_fail_point_arg("attest-ignore-auth-failure", move || {
                (iota_simulator::current_simnode_id() == attestor_sim_id).then_some(claimed_units)
            });
            if let Some(versions) = recorded_versions.clone() {
                register_fail_point_arg("attest-object-versions", move || {
                    (iota_simulator::current_simnode_id() == attestor_sim_id)
                        .then(|| versions.clone())
                });
            }
            let digest = self.submit_to(tx, attestor).await;
            clear_fail_point("attest-ignore-auth-failure");
            if recorded_versions.is_some() {
                clear_fail_point("attest-object-versions");
            }

            digest
        }

        /// Asserts what the nodes recorded for the attestation of the
        /// scenario's transaction. `certified` is the record that the
        /// transaction's checkpoint certifies, and it must match the scenario.
        /// Every validator judged the attestation at execution and must have
        /// stored that record, or a [`AttestationVerdict::Valid`] one where
        /// none is certified. The fullnode executes from checkpoints, so it
        /// must have stored the certified record, and none for a valid
        /// attestation.
        fn assert_records(&self, scenario: &Scenario, certified: Option<AttestationRecord>) {
            let attestor = self.authority_index(&scenario.attestor);
            let certified_verdict = certified.map(|record| record.verdict);
            let matches_scenario = match scenario.expected_verdict {
                ExpectedVerdict::Valid => certified_verdict.is_none(),
                ExpectedVerdict::Refuted | ExpectedVerdict::RefutedByRerun => matches!(
                    certified_verdict,
                    Some(AttestationVerdict::Refuted { units }) if units > 0
                ),
            };
            assert!(
                matches_scenario && certified.is_none_or(|record| record.attestor == attestor),
                "{}: expected {:?} by attestor {attestor}, certified {certified:?}",
                scenario.name,
                scenario.expected_verdict
            );
            let judged = certified.unwrap_or(AttestationRecord {
                attestor,
                verdict: AttestationVerdict::Valid,
            });
            for handle in self.test_cluster.all_validator_handles() {
                let (name, record) = handle.with(|node| {
                    (
                        node.state().name,
                        read_attestation_record(node, &scenario.digest),
                    )
                });
                assert_eq!(
                    record,
                    Some(judged),
                    "{}: validator {} must record the certified verdict",
                    scenario.name,
                    name.concise()
                );
            }
            let on_fullnode = self
                .test_cluster
                .fullnode_handle
                .iota_node
                .with(|node| read_attestation_record(node, &scenario.digest));
            assert_eq!(
                on_fullnode, certified,
                "{}: the fullnode copies the certified record",
                scenario.name
            );
        }
    }

    /// Asserts that the scenario's transaction failed Move authentication at
    /// execution if the scenario's `authenticates` is false, and succeeded if
    /// it is true.
    fn assert_status(scenario: &Scenario, effects: &TransactionEffects) {
        let status = effects.status();

        if scenario.authenticates {
            assert!(
                status.is_success(),
                "{}: expected success, got {status:?}",
                scenario.name
            );
        } else {
            assert!(
                matches!(
                    status,
                    ExecutionStatus::Failure {
                        error: ExecutionError::MoveAuthentication { .. },
                        ..
                    }
                ),
                "{}: expected a Move authentication failure, got {status:?}",
                scenario.name
            );
        }
    }

    /// Asserts who the effects charge for the scenario's transaction, as its
    /// [`ExpectedVerdict`] says. `gas_coin` is the transaction's gas coin as
    /// the fullnode holds it after the execution, which must be at the
    /// reference the effects name. When the gas owner pays, its balance is
    /// [`GAS_COIN_BALANCE`] less the charge and what the body moved. When
    /// nobody pays, its balance is untouched.
    fn assert_payment(scenario: &Scenario, effects: &TransactionEffects, gas_coin: &Object) {
        assert_eq!(
            &gas_coin.object_ref(),
            effects.gas_object().reference(),
            "{}: the fullnode holds the gas coin as the effects name it",
            scenario.name
        );
        let summary = effects.gas_cost_summary();
        let balance = gas_coin
            .as_coin_maybe()
            .expect("the gas object is a coin")
            .value();
        match scenario.expected_verdict {
            ExpectedVerdict::Valid => {
                assert!(
                    summary.gas_used() > 0,
                    "{}: the gas owner must be charged, got {summary:?}",
                    scenario.name
                );
                let charge = u64::try_from(summary.net_gas_usage())
                    .expect("the charge exceeds the storage rebate");
                let moved_by_the_body = if scenario.authenticates {
                    BODY_TRANSFER_AMOUNT
                } else {
                    0
                };
                assert_eq!(
                    balance,
                    GAS_COIN_BALANCE - charge - moved_by_the_body,
                    "{}: the gas coin must pay the charge",
                    scenario.name
                );
            }
            ExpectedVerdict::Refuted | ExpectedVerdict::RefutedByRerun => {
                assert_eq!(
                    summary,
                    &GasCostSummary::default(),
                    "{}: nobody pays for a refuted attestation",
                    scenario.name
                );
                assert_eq!(
                    balance, GAS_COIN_BALANCE,
                    "{}: the gas coin's balance stays untouched",
                    scenario.name
                );
            }
        }
    }

    /// Reads the node's record of the attestation of the transaction `digest`,
    /// executed in the node's current epoch.
    fn read_attestation_record(
        node: &iota_node::IotaNode,
        digest: &TransactionDigest,
    ) -> Option<AttestationRecord> {
        let state = node.state();

        state
            .get_transaction_cache_reader()
            .multi_get_attestation_records_in_epoch(
                state.epoch_store_for_testing().epoch(),
                &[*digest],
            )
            .pop()
            .expect("one result per digest")
    }

    /// Waits until every validator and the fullnode have executed the
    /// transaction `digest`, and returns the effects on each. Panics if a node
    /// does not execute it within 60 seconds.
    async fn wait_for_effects_on_every_node(
        test_cluster: &TestCluster,
        digest: TransactionDigest,
    ) -> Vec<TransactionEffects> {
        let mut effects = Vec::new();
        for handle in test_cluster.all_node_handles() {
            let on_node = timeout(
                Duration::from_secs(60),
                handle.with_async(|node| async move {
                    node.state()
                        .get_transaction_cache_reader()
                        .try_notify_read_executed_effects("", &[digest])
                        .await
                }),
            )
            .await
            .expect("every node must execute the transaction within 60s")
            .expect("the executed effects must be readable")
            .remove(0);
            effects.push(on_node);
        }

        effects
    }

    fn highest_executed_checkpoint(handle: &IotaNodeHandle) -> CheckpointSequenceNumber {
        handle.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    }

    /// The highest checkpoint the validator built in its current epoch.
    fn last_built_checkpoint(handle: &IotaNodeHandle) -> CheckpointSequenceNumber {
        handle.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .last_built_checkpoint_builder_summary()
                .unwrap()
                .map(|summary| summary.summary.sequence_number)
                .unwrap_or_default()
        })
    }

    /// Waits until every validator has executed and built the checkpoint five
    /// past the one the fullnode has executed. Then asserts that each
    /// validator's own summary of every checkpoint matches the certified one,
    /// for the checkpoints whose own summary the validator still holds: in
    /// practice those of its current epoch.
    async fn assert_validators_build_the_certified_checkpoints(test_cluster: &TestCluster) {
        let target = highest_executed_checkpoint(&test_cluster.fullnode_handle.iota_node) + 5;
        let validators = test_cluster.all_validator_handles();
        timeout(Duration::from_secs(180), async {
            while validators.iter().any(|handle| {
                highest_executed_checkpoint(handle) < target
                    || last_built_checkpoint(handle) < target
            }) {
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("every validator must execute and build checkpoint {target}"));
        for handle in &validators {
            handle.with(|node| {
                let store = node.state().get_checkpoint_store().clone();
                for sequence_number in 1..=target {
                    let local = store
                        .get_locally_computed_checkpoint(sequence_number)
                        .unwrap();
                    let certified = store
                        .get_checkpoint_by_sequence_number(sequence_number)
                        .unwrap();
                    if let (Some(local), Some(certified)) = (local, certified) {
                        assert_eq!(
                            local.digest(),
                            *certified.digest(),
                            "validator {}'s checkpoint {sequence_number} must match the certified one",
                            node.state().name.concise(),
                        );
                    }
                }
            });
        }
    }

    /// Reads what the fullnode's checkpoints of `epoch` certify about
    /// attestations, from the epoch's first checkpoint through `last`: the
    /// record of each transaction whose record is not
    /// [`AttestationVerdict::Valid`], and the totals at `last`. Panics if the
    /// fullnode lacks one of the checkpoints, or unless each checkpoint's
    /// totals add the squared units of its own refuted records to the
    /// previous checkpoint's totals.
    fn read_certified_attestations(
        test_cluster: &TestCluster,
        epoch: EpochId,
        last: CheckpointSequenceNumber,
    ) -> (
        BTreeMap<TransactionDigest, AttestationRecord>,
        AttestationTotals,
    ) {
        test_cluster.fullnode_handle.iota_node.with(|node| {
            let state = node.state();
            let store = state.get_checkpoint_store();
            let protocol_config = state.epoch_store_for_testing().protocol_config().clone();
            let first = match epoch.checked_sub(1) {
                Some(previous) => {
                    store
                        .get_epoch_last_checkpoint_seq_number(previous)
                        .unwrap()
                        .expect("the previous epoch ended")
                        + 1
                }
                None => 0,
            };
            let mut records = BTreeMap::new();
            let mut expected_totals = BTreeMap::new();
            let mut totals = AttestationTotals::default();
            for sequence_number in first..=last {
                let checkpoint = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap()
                    .expect("the fullnode holds the checkpoints of the epoch");
                assert_eq!(checkpoint.epoch, epoch);
                let contents = store
                    .get_checkpoint_contents(&checkpoint.contents_digest)
                    .unwrap()
                    .expect("the fullnode holds the contents of its checkpoints");
                let data = checkpoint
                    .parse_version_specific_data(&protocol_config)
                    .unwrap()
                    .expect("summaries carry version specific data");
                for entry in data.attestation_entries() {
                    let digest = contents.transactions()[entry.position as usize].transaction;
                    records.insert(digest, entry.record);
                    if let AttestationVerdict::Refuted { units } = entry.record.verdict {
                        *expected_totals.entry(entry.record.attestor).or_default() += units.pow(2);
                    }
                }
                totals = data
                    .attestation_totals()
                    .cloned()
                    .expect("summaries carry the attestation totals");
                assert_eq!(
                    totals.refuted_units_squared, expected_totals,
                    "checkpoint {sequence_number} must carry the squared units of every refuted \
                     attestation of the epoch so far"
                );
            }

            (records, totals)
        })
    }

    /// Reads the [`ChangeEpochV4::scores`] of the change epoch transaction that
    /// ended `epoch`, in the order of the epoch's [`committee_validators()`],
    /// the order Move reads them in.
    ///
    /// [`ChangeEpochV4::scores`]: iota_sdk_types::ChangeEpochV4::scores
    /// [`committee_validators()`]: EpochStartSystemStateTrait::committee_validators
    fn read_change_epoch_scores(test_cluster: &TestCluster, epoch: EpochId) -> Vec<u64> {
        test_cluster.fullnode_handle.iota_node.with(|node| {
            let state = node.state();
            let store = state.get_checkpoint_store();
            let checkpoint = store
                .get_epoch_last_checkpoint(epoch)
                .unwrap()
                .expect("the epoch ended");
            let contents = store
                .get_checkpoint_contents(&checkpoint.contents_digest)
                .unwrap()
                .expect("the fullnode holds the contents of its checkpoints");
            // The epoch's last checkpoint ends with the change epoch
            // transaction.
            let digest = contents
                .transactions()
                .last()
                .expect("the epoch's last checkpoint is not empty")
                .transaction;
            let transaction = state
                .get_transaction_cache_reader()
                .get_transaction_block(&digest)
                .expect("the fullnode holds the change epoch transaction");
            let TransactionKind::EndOfEpoch(kinds) = transaction.data().transaction().kind() else {
                panic!("the epoch's last transaction ends the epoch");
            };

            kinds
                .iter()
                .find_map(EndOfEpochTransactionKind::as_opt_change_epoch_v4)
                .expect("the change epoch transaction passes the scores")
                .scores
                .clone()
        })
    }

    /// Reads the node's `checkpoint_exec_attestation_verdict_mismatches`
    /// counter: how many executed transactions the node recorded with a
    /// verdict that differs from the certified one.
    fn read_verdict_mismatches(handle: &IotaNodeHandle) -> f64 {
        handle
            .with(|node| node.gather_metrics())
            .iter()
            .find(|family| family.name() == "checkpoint_exec_attestation_verdict_mismatches")
            .expect("every node counts the verdict mismatches")
            .get_metric()
            .iter()
            .map(|metric| metric.get_counter().value())
            .sum()
    }
}
