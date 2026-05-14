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
//!
//! # Authority aggregator access when WFF is enabled
//!
//! When `enable_white_flag_flow` is `true`, `TransactionOrchestrator` stores
//! the aggregator in `TransactionDriver`, not in `QuorumDriverHandler`.
//! `TestCluster::authority_aggregator()` panics in this configuration because
//! it always goes through `quorum_driver()`.  We therefore reach the
//! aggregator via `transaction_driver()` in `submit_tx_v2`.
//!
//! For the same reason, setup transactions must go through the wallet JSON-RPC
//! path (`test_cluster.execute_transaction`) rather than
//! `execute_transaction_return_raw_effects`, which internally calls
//! `authority_aggregator()`.

use std::net::SocketAddr;

use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_core::authority_client::validator_v2::ValidatorV2API;
use iota_json_rpc_types::ObjectChange;
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{Identifier, IotaAddress, ObjectID, ObjectRef},
    messages_grpc::TxStatusUpdate,
    move_authenticator::MoveAuthenticator,
    move_package,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{
        Argument, CallArg, ProgrammableTransaction, SharedObjectRef,
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction, TransactionData,
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
    let _env = ProtocolEnvOverride::new(&[
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1"),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION",
            "true",
        ),
    ]);

    // Build a test environment and create an abstract account.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.object_id.into();

    // Fund the AA with gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Build a simple AA transaction.
    let pt = test_env.craft_aa_simple_ptb()?;
    let tx_data = test_env.craft_tx_from_pt(pt, aa_gas, aa_sender).await?;
    let tx_digest = tx_data.digest().into_inner();

    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let aa_tx = Transaction::from_generic_sig_data(tx_data, signatures);

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

// --------------------------------------------------
// --- Protocol config env override RAII guard ------
// --------------------------------------------------

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
            // Set before any node thread is spawned; no concurrent env readers at this
            // point.
            #[allow(deprecated)]
            std::env::set_var(key, val);
        }
        Self {
            keys: overrides.iter().map(|(k, _)| *k).collect(),
        }
    }
}

impl Drop for ProtocolEnvOverride {
    fn drop(&mut self) {
        for key in &self.keys {
            #[allow(deprecated)]
            std::env::remove_var(key);
        }
    }
}

// --------------------------------------------------
// --- Minimal test environment ---------------------
// --------------------------------------------------

struct TestEnvironment {
    test_cluster: TestCluster,
    owner: Option<IotaAddress>,
    aa_package_id: Option<ObjectID>,
    aa_package_metadata_ref: Option<ObjectRef>,
    aa_ref: Option<ObjectRef>,
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

        // Use the wallet JSON-RPC path so we don't call authority_aggregator(),
        // which panics when white-flag flow is active (QuorumDriverHandler is None).
        let response = self.test_cluster.execute_transaction(transaction).await;

        self.aa_ref = response
            .object_changes
            .as_ref()
            .expect("object_changes must be populated")
            .iter()
            .find_map(|change| {
                if let ObjectChange::Created {
                    object_id,
                    version,
                    digest,
                    owner: Owner::Shared { .. },
                    ..
                } = change
                {
                    Some(iota_types::base_types::ObjectRef::new(
                        *object_id, *version, *digest,
                    ))
                } else {
                    None
                }
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
    ) -> anyhow::Result<Transaction> {
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
            builder.obj(CallArg::Shared(SharedObjectRef {
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
            vec![
                iota_types::base_types::TypeTag::U8,
                iota_types::base_types::TypeTag::U8,
            ],
            arguments,
        );
        Ok(builder.finish())
    }

    async fn craft_tx_from_pt(
        &self,
        pt: ProgrammableTransaction,
        gas_coin: ObjectRef,
        sender: IotaAddress,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.test_cluster.get_reference_gas_price().await;
        Ok(TransactionData::new_programmable_allow_sponsor(
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
    ) -> anyhow::Result<GenericSignature> {
        let (Some(aa_ref), Some(owner)) = (self.aa_ref, self.owner) else {
            anyhow::bail!("Abstract account not set up yet");
        };
        let signature = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_hashed(&owner, tx_digest)?;

        let hex_encoded_signature: String = Hex::encode(&signature)
            .chars()
            .skip(2)
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
        let self_call_arg = CallArg::Shared(SharedObjectRef {
            object_id: aa_ref.object_id,
            initial_shared_version: aa_ref.version,
            mutable: false,
        });
        let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![signature_call_arg], vec![], self_call_arg),
        ))
    }

    /// Submit a transaction via the V2 gRPC path on the first available
    /// validator.
    ///
    /// When `enable_white_flag_flow` is on, `TransactionOrchestrator` stores
    /// the aggregator in `TransactionDriver` (not `QuorumDriverHandler`).
    /// We select the right source at runtime.
    async fn submit_tx_v2(
        &self,
        tx: Transaction,
    ) -> Result<
        Vec<(iota_types::digests::TransactionDigest, TxStatusUpdate)>,
        iota_types::error::IotaError,
    > {
        let client = self.test_cluster.fullnode_handle.iota_node.with(|node| {
            let orchestrator = node
                .transaction_orchestrator()
                .expect("TransactionOrchestrator not initialised on fullnode");

            // When WFF is enabled TransactionDriver holds the aggregator;
            // QuorumDriverHandler is None and clone_authority_aggregator() would panic.
            let agg = if let Some(td) = orchestrator.transaction_driver() {
                td.authority_aggregator().load_full()
            } else {
                orchestrator.clone_authority_aggregator()
            };

            agg.authority_clients
                .values()
                .next()
                .expect("No authority clients")
                .authority_client()
                .clone()
        });

        client
            .submit_tx(vec![tx], Some(SocketAddr::new([127, 0, 0, 1].into(), 0)))
            .await
    }
}

fn abstract_account_type_tag(aa_package_id: &ObjectID) -> iota_types::base_types::TypeTag {
    use std::str::FromStr;
    iota_types::base_types::TypeTag::from_str(&format!(
        "{aa_package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}"
    ))
    .unwrap()
}
