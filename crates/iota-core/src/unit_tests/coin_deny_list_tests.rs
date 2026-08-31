// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Identifier, ObjectId, ObjectReference, SharedObjectReference, StructTag,
    TransactionDigest, TransactionEffects, TypeTag, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_account_private_key},
    deny_list_v1::{
        DenyCapV1, RegulatedCoinMetadata, check_address_denied_by_config, check_global_pause,
        get_per_type_coin_deny_list_v1,
    },
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult, UserInputError},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TransactionEnvelope, VerifiedTransaction,
    },
};

use crate::{
    authority::{
        AuthorityState, authority_test_utils::send_and_confirm_transaction_with_execution_error,
        authority_tests::send_and_confirm_transaction_,
        move_integration_tests::build_and_try_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_handler::VerifiedSequencedConsensusTransaction,
    post_consensus_validation,
};

// Test that a v1 regulated coin can be created and all the necessary objects
// are created with the right types. Also test that we could create the deny
// list config for the coin and all types can be loaded in Rust.
#[tokio::test]
async fn test_regulated_coin_v1_types() {
    let env = new_authority_and_publish("coin_deny_list_v1").await;

    // Step 1: Publish the regulated coin and check basic types.
    let mut deny_cap_object = None;
    let mut metadata_object = None;
    let mut regulated_metadata_object = None;
    let mut package_id = None;
    for created in env.publish_effects.created() {
        let object = env
            .authority
            .get_object(&created.reference.object_id)
            .unwrap();
        if object.is_package() {
            package_id = Some(object.id());
            continue;
        }
        let t = object.data.opt_object_type().unwrap();
        if t.is_deny_cap_v1() {
            assert!(deny_cap_object.is_none());
            deny_cap_object = Some(object);
        } else if t.is_regulated_coin_metadata() {
            assert!(regulated_metadata_object.is_none());
            regulated_metadata_object = Some(object);
        } else if t.is_coin_metadata() {
            assert!(metadata_object.is_none());
            metadata_object = Some(object);
        }
    }
    let package_id = package_id.unwrap();
    // Check that publishing the package created
    // the metadata, deny cap, and regulated metadata.
    // Check that all their fields are consistent.
    let metadata_object = metadata_object.unwrap();
    let deny_cap_object = deny_cap_object.unwrap();
    let deny_cap: DenyCapV1 = deny_cap_object.to_rust().unwrap();
    assert_eq!(deny_cap.id.id.bytes, deny_cap_object.id());
    assert!(deny_cap.allow_global_pause);

    let regulated_metadata_object = regulated_metadata_object.unwrap();
    let regulated_metadata: RegulatedCoinMetadata = regulated_metadata_object.to_rust().unwrap();
    assert_eq!(
        regulated_metadata.id.id.bytes,
        regulated_metadata_object.id()
    );
    assert_eq!(
        regulated_metadata.deny_cap_object.bytes,
        deny_cap_object.id()
    );
    assert_eq!(
        regulated_metadata.coin_metadata_object.bytes,
        metadata_object.id()
    );

    // Step 2: Deny an address and check the denylist types.
    let deny_list_object_init_version = env
        .get_latest_object_ref(&ObjectId::DENY_LIST)
        .await
        .version;
    let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
        package_id,
        Identifier::from_static("regulated_coin"),
        Identifier::from_static("REGULATED_COIN"),
        vec![],
    )));
    let deny_address = dbg_addr(2);
    let tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        env.authority.reference_gas_price_for_testing().unwrap(),
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_add",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(deny_cap_object.object_ref()),
            CallArg::pure(&deny_address),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.private_key);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, tx, true)
        .await
        .unwrap();
    if effects.status().is_failure() {
        panic!("Failed to add address to deny list: {:?}", effects.status());
    }
    let coin_deny_config = get_per_type_coin_deny_list_v1(
        &regulated_coin_type.to_canonical_string(false),
        &env.authority.get_object_store(),
    )
    .unwrap();
    // Updates from the current epoch will not be read.
    assert!(!check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        Some(0),
    ));
    // If no epoch is specified, we always read the latest value, and it should be
    // denied.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        None,
    ));
    // If no epoch is specified, we always read the latest value, and it should be
    // denied.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        None,
    ));

    // If we change the current epoch to be 1, the change from epoch 0
    // would be considered as from previous epoch, and hence will be
    // used.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        Some(1),
    ));
    // Check a different address, and it should not be denied.
    assert!(!check_address_denied_by_config(
        &coin_deny_config,
        dbg_addr(3),
        &env.authority.get_object_store(),
        Some(1),
    ));

    // Step 3: Enable global pause and check the global pause types.
    let tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        env.authority.reference_gas_price_for_testing().unwrap(),
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_enable_global_pause",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(env.get_latest_object_ref(&deny_cap_object.id()).await),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.private_key);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, tx, true)
        .await
        .unwrap();
    if effects.status().is_failure() {
        panic!("Failed to enable global pause: {:?}", effects.status());
    }
    println!("Effects: {effects:?}");
    assert!(check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        None,
    ));
    assert!(!check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        Some(0),
    ));
    assert!(check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        Some(1),
    ));
}

// The coin deny-list read mode of `handle_transaction_validation_checks`
// decides whether its verdict depends on the validator's execution progress.
//
// With the latest-value read (`epoch_gated_coin_deny_list = false`), the
// verdict for one and the same transaction flips within an epoch once the
// `deny_list_v1_add` transaction executes locally. That is intended for
// admission at signing (denials apply immediately), but post-consensus it
// would make two validators - one that has already executed the deny-add and
// one that has not - reach opposite keep/drop decisions for the same
// sequenced transaction, diverging the checkpoint. The epoch-gated read
// (`epoch_gated_coin_deny_list = true`) returns the value settled before the
// current epoch and is identical at both frontiers, which is why the
// post-consensus caller uses it.
//
// The two calls below (before/after executing the deny-add on a single
// authority) model those two execution frontiers.
#[tokio::test]
async fn test_coin_deny_list_read_modes_across_execution_progress() {
    let env = RegulatedCoinEnv::new().await;
    assert_eq!(env.epoch(), 0);
    let transfer_tx = env.build_transfer().await;

    // Frontier 1 - the deny-add has not executed: no deny list config exists
    // for the coin type, both read modes pass the transaction.
    for epoch_gated in [false, true] {
        env.validation_check(&transfer_tx, epoch_gated)
            .await
            .expect("no deny list entry executed yet");
    }
    assert!(
        get_per_type_coin_deny_list_v1(
            &env.regulated_coin_type.to_canonical_string(false),
            &env.env.authority.get_object_store(),
        )
        .is_none()
    );

    // Execute `deny_list_v1_add(sender)` in the same epoch.
    env.deny_sender().await;

    // Frontier 2, latest-value read - the very same transaction is now
    // rejected within the same epoch: the verdict followed the local
    // execution frontier.
    let err = env.validation_check(&transfer_tx, false).await.unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::AddressDeniedForCoin { address, .. }
            } if *address == env.env.sender
        ),
        "unexpected error: {err:?}"
    );

    // Frontier 2, epoch-gated read - same verdict as frontier 1: the entry
    // written this epoch is not active yet, execution progress is irrelevant.
    env.validation_check(&transfer_tx, true)
        .await
        .expect("entry written this epoch must not be active for the epoch-gated read");
}

// Relaxation mirror of the test above: a denial settled in a previous epoch
// is lifted in the current one. The latest-value read honors the removal
// immediately, while the epoch-gated read still denies until the removal
// settles at the next epoch boundary - the window in which admission accepts
// transactions that post-consensus deterministically drops.
#[tokio::test]
async fn test_coin_deny_list_read_modes_after_denial_relaxed() {
    let env = RegulatedCoinEnv::new().await;
    env.deny_sender().await;

    // Settle the denial: entries written in epoch 0 activate in epoch 1.
    env.reconfigure().await;
    assert_eq!(env.epoch(), 1);

    let transfer_tx = env.build_transfer().await;

    // Frontier 1 - the removal has not executed: the denial is settled, both
    // read modes reject the transaction.
    for epoch_gated in [false, true] {
        let err = env
            .validation_check(&transfer_tx, epoch_gated)
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                IotaError::UserInput {
                    error: UserInputError::AddressDeniedForCoin { .. }
                }
            ),
            "a settled denial must be enforced by both read modes, got {err:?}"
        );
    }

    // Execute `deny_list_v1_remove(sender)` in the same epoch.
    env.undeny_sender().await;

    // Frontier 2, latest-value read - the removal applies immediately.
    env.validation_check(&transfer_tx, false)
        .await
        .expect("a removal must apply immediately for the latest-value read");

    // Frontier 2, epoch-gated read - the removal is not settled yet, the
    // denial still holds until the next epoch boundary.
    let err = env.validation_check(&transfer_tx, true).await.unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::AddressDeniedForCoin { .. }
            }
        ),
        "a removal written this epoch must not be active for the epoch-gated read, got {err:?}"
    );
}

// Runs the real post-consensus validation pipeline over a `UserTransactionV1`
// spending a regulated coin whose type was globally paused earlier in the
// same epoch. The pause has already executed locally, so the latest-value
// read would report "paused" and drop the transaction - on this validator,
// but not on one whose execution lags behind the pause. The epoch-gated read
// used post-consensus keeps it on every validator: the pause activates next
// epoch. (Recipient-side enforcement at execution,
// `check_coin_deny_list_v1_during_execution`, has always been epoch-gated
// the same way.)
#[tokio::test]
async fn test_post_consensus_keeps_tx_spending_coin_paused_this_epoch() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let env = RegulatedCoinEnv::new().await;

    // Globally pause the regulated coin; executes in the current epoch.
    env.pause().await;

    // A transaction spending the paused coin, sequenced as `UserTransactionV1`.
    let transfer_tx = env.build_transfer().await;
    let (dropped, kept) = env.post_consensus_validate(transfer_tx).await;
    assert!(dropped.is_empty(), "unexpected drops: {dropped:?}");
    assert!(kept);
}

// The admitted-then-dropped window end-to-end: a pause settled at the epoch
// boundary is lifted mid-epoch. Admission (latest-value read) honors the
// lifted pause immediately, while the epoch-gated post-consensus read still
// sees the settled pause, so the admitted transaction is deterministically
// dropped until the removal settles at the next epoch boundary.
#[tokio::test]
async fn test_post_consensus_drops_tx_spending_coin_unpaused_this_epoch() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let env = RegulatedCoinEnv::new().await;
    env.pause().await;

    // `reconfigure_for_testing` applies its own config override (carrying the
    // current epoch store's config, including the flag set above, into the
    // next epoch) and panics if another override is still installed.
    drop(guard);

    // Settle the pause: entries written in epoch 0 activate in epoch 1.
    env.reconfigure().await;
    assert_eq!(env.epoch(), 1);

    // Lift the pause; executes in epoch 1, settles at epoch 2.
    env.unpause().await;

    let transfer_tx = env.build_transfer().await;

    // Admission honors the lifted pause immediately.
    env.validation_check(&transfer_tx, false)
        .await
        .expect("a lifted pause must apply immediately at admission");

    // Post-consensus still sees the settled pause and drops.
    let (dropped, kept) = env.post_consensus_validate(transfer_tx).await;
    assert!(!kept);
    assert_eq!(dropped.len(), 1);
    assert!(
        matches!(
            &dropped[0].1,
            IotaError::UserInput {
                error: UserInputError::CoinTypeGlobalPause { .. }
            }
        ),
        "unexpected drop reason: {:?}",
        dropped[0].1
    );
}

/// A published `coin_deny_list_v1_mintable` package - the regulated coin
/// type, its deny cap, and one minted coin - with helpers for the deny-list
/// operations the tests exercise.
struct RegulatedCoinEnv {
    env: TestEnv,
    regulated_coin_type: TypeTag,
    deny_cap_id: ObjectId,
    coin_id: ObjectId,
    /// Captured before any deny-list mutation: `CallArg::Shared` needs the
    /// initial shared version, which later mutations do not change.
    deny_list_initial_version: Version,
}

impl RegulatedCoinEnv {
    async fn new() -> Self {
        let env = new_authority_and_publish("coin_deny_list_v1_mintable").await;
        let mut package_id = None;
        let mut deny_cap_id = None;
        let mut coin_id = None;

        for created in env.publish_effects.created() {
            let object = env
                .authority
                .get_object(&created.reference.object_id)
                .unwrap();
            if object.is_package() {
                package_id = Some(object.id());
                continue;
            }
            if object.data.opt_object_type().unwrap().is_deny_cap_v1() {
                deny_cap_id = Some(object.id());
            } else if !object.is_gas_coin() && object.opt_coin_type().is_some() {
                coin_id = Some(object.id());
            }
        }

        let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
            package_id.expect("package must be created"),
            Identifier::from_static("regulated_coin"),
            Identifier::from_static("REGULATED_COIN"),
            vec![],
        )));
        let deny_list_initial_version = env
            .get_latest_object_ref(&ObjectId::DENY_LIST)
            .await
            .version;

        Self {
            env,
            regulated_coin_type,
            deny_cap_id: deny_cap_id.expect("deny cap must be created"),
            coin_id: coin_id.expect("minted regulated coin must be created"),
            deny_list_initial_version,
        }
    }

    fn rgp(&self) -> u64 {
        self.env
            .authority
            .reference_gas_price_for_testing()
            .unwrap()
    }

    fn epoch(&self) -> u64 {
        self.env.authority.epoch_store_for_testing().epoch()
    }

    async fn reconfigure(&self) {
        self.env.authority.reconfigure_for_testing().await;
    }

    /// Executes a `0x2::coin` deny-list function over the regulated coin
    /// type; `deny_list_v1_add`/`deny_list_v1_remove` take a target address,
    /// the global pause functions take none.
    async fn call_deny_list(&self, function: &'static str, address: Option<Address>) {
        let mut args = vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                self.deny_list_initial_version,
                true,
            )),
            // Re-fetched every call: the cap's version bumps on each one.
            CallArg::ImmutableOrOwned(self.env.get_latest_object_ref(&self.deny_cap_id).await),
        ];

        if let Some(address) = address {
            args.push(CallArg::pure(&address));
        }

        let tx = TestTransactionBuilder::new(
            self.env.sender,
            self.env
                .get_latest_object_ref(&self.env.gas_object_id)
                .await,
            self.rgp(),
        )
        .move_call(ObjectId::FRAMEWORK, "coin", function, args)
        .with_type_args(vec![self.regulated_coin_type.clone()])
        .build_and_sign(&self.env.private_key);

        // `fake_consensus = false`: assign the shared-object version directly
        // instead of going through the consensus commit handler, which
        // requires a randomness manager - the epoch store created by
        // `reconfigure_for_testing` has none.
        let (_, effects, _) = send_and_confirm_transaction_with_execution_error(
            &self.env.authority,
            None,
            tx,
            true,
            false,
        )
        .await
        .unwrap();

        assert!(
            !effects.status().is_failure(),
            "{function}: {:?}",
            effects.status()
        );
    }

    async fn deny_sender(&self) {
        self.call_deny_list("deny_list_v1_add", Some(self.env.sender))
            .await;
    }

    async fn undeny_sender(&self) {
        self.call_deny_list("deny_list_v1_remove", Some(self.env.sender))
            .await;
    }

    async fn pause(&self) {
        self.call_deny_list("deny_list_v1_enable_global_pause", None)
            .await;
    }

    async fn unpause(&self) {
        self.call_deny_list("deny_list_v1_disable_global_pause", None)
            .await;
    }

    /// A transfer of the regulated coin with its own freshly inserted gas
    /// object, so the deny-list calls above (which spend the publisher's gas)
    /// never invalidate its input references.
    async fn build_transfer(&self) -> TransactionEnvelope {
        let gas_object = Object::with_owner_for_testing(self.env.sender);
        self.env.authority.insert_genesis_object(gas_object.clone());

        TestTransactionBuilder::new(self.env.sender, gas_object.object_ref(), self.rgp())
            .transfer(
                self.env.get_latest_object_ref(&self.coin_id).await,
                dbg_addr(2),
            )
            .build_and_sign(&self.env.private_key)
    }

    /// Runs `handle_transaction_validation_checks` with the given coin
    /// deny-list read mode. The epoch store is fetched per call, so the check
    /// follows epoch changes made through [`Self::reconfigure`].
    async fn validation_check(
        &self,
        transaction: &TransactionEnvelope,
        epoch_gated_coin_deny_list: bool,
    ) -> IotaResult<Vec<ObjectReference>> {
        let epoch_store = self.env.authority.epoch_store_for_testing();

        self.env
            .authority
            .handle_transaction_validation_checks(
                &VerifiedTransaction::new_unchecked(transaction.clone()),
                &epoch_store,
                &self.env.authority.config.transaction_deny_config,
                epoch_gated_coin_deny_list,
            )
            .await
    }

    /// Runs post-consensus validation over the transaction sequenced as a
    /// `UserTransactionV1`. Returns the drop list and whether the transaction
    /// was kept in the sequence.
    async fn post_consensus_validate(
        &self,
        transaction: TransactionEnvelope,
    ) -> (Vec<(TransactionDigest, IotaError)>, bool) {
        let digest = *transaction.digest();
        let consensus_tx = ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(transaction)),
            tracking_id: Default::default(),
        };

        let mut transactions = vec![VerifiedSequencedConsensusTransaction::new_test(
            consensus_tx,
        )];

        let epoch_store = self.env.authority.epoch_store_for_testing();
        let (dropped, _locks, user_tx_digests) =
            post_consensus_validation::validate_and_resolve_conflicts(
                &self.env.authority,
                &epoch_store,
                &mut transactions,
            )
            .await
            .unwrap();

        assert_eq!(user_tx_digests, vec![digest]);

        (dropped, transactions.len() == 1)
    }
}

struct TestEnv {
    authority: Arc<AuthorityState>,
    sender: Address,
    private_key: AccountPrivateKey,
    gas_object_id: ObjectId,
    publish_effects: TransactionEffects,
}

impl TestEnv {
    async fn get_latest_object_ref(&self, id: &ObjectId) -> ObjectReference {
        self.authority.get_object(id).unwrap().object_ref()
    }
}

async fn new_authority_and_publish(path: &str) -> TestEnv {
    let (sender, sender_key) = get_account_private_key();
    let gas_object = Object::with_owner_for_testing(sender);
    let gas_object_id = gas_object.id();
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&[gas_object])
        .build()
        .await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let (_, effects) = build_and_try_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas_object_id,
        path,
        TEST_ONLY_GAS_UNIT_FOR_PUBLISH * rgp,
        rgp,
        false,
    )
    .await;
    TestEnv {
        authority,
        sender,
        private_key: sender_key,
        gas_object_id,
        publish_effects: effects.into_data(),
    }
}
