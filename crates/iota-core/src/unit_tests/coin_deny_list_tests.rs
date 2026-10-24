// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_move_types::iota_framework::coin::{DenyCapV1, RegulatedCoinMetadata};
use iota_sdk_types::{
    Address, ExecutionStatus, Identifier, ObjectId, ObjectReference, Owner, SharedObjectReference,
    StructTag, TransactionEffects, TypeTag, UnchangedSharedKind, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_account_private_key},
    deny_list_v1::{
        check_address_denied_by_config, check_global_pause, get_per_type_coin_deny_list_v1,
    },
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{ExecutionErrorKind, IotaError, IotaResult, UserInputError},
    object::Object,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TransactionEnvelope, TransactionKey,
        VerifiedTransaction,
    },
};

use crate::{
    authority::{
        AuthorityState, authority_per_epoch_store::handler_object_state::CommitIndex,
        authority_test_utils::send_and_confirm_transaction_with_execution_error,
        authority_tests::send_and_confirm_transaction_,
        move_integration_tests::build_and_try_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    post_consensus_input_reader::{ValidationAtCommit, reader::CommitIndexedReader},
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
            .get_object(&created.reference().object_id)
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
    let deny_cap: DenyCapV1<()> = deny_cap_object.to_rust().unwrap();
    assert_eq!(deny_cap.id.id.bytes, deny_cap_object.id());
    assert!(deny_cap.allow_global_pause);

    let regulated_metadata_object = regulated_metadata_object.unwrap();
    let regulated_metadata: RegulatedCoinMetadata<()> =
        regulated_metadata_object.to_rust().unwrap();
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

// Post-consensus validation at commit C reads state as of commit C - K,
// called the horizon of C. K is the protocol config's
// `pcool_deterministic_validation_horizon_distance`, 4 in these tests, so
// commit 4 has horizon 0 and commit 5 has horizon 1.
//
// Under P-COOL, post-consensus validation checks the sender against the coin
// types of its inputs, reading the deny list as of the commit's horizon. A
// sender denied in the previous epoch and allowed again at commit 1 of this one
// is admitted, since admission reads the latest deny list. The removal counts
// from commit 5 (horizon 1) on: commit 4 drops the transaction, commit 5 keeps
// it, and execution, which no longer checks the inputs, runs it.
#[tokio::test]
async fn test_sender_undenied_this_epoch_is_kept_once_the_horizon_reaches_the_removal() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    env.deny_sender().await;
    drop(epoch_zero);

    let _guard = env.reconfigure_with_the_pcool_flow().await;
    env.undeny_sender_at(1).await;

    let transfer_tx = env.build_transfer().await;
    env.validation_check(&transfer_tx)
        .await
        .expect("admission must accept a sender the latest deny list allows");
    env.assert_dropped_at_commit(
        &transfer_tx,
        4,
        UserInputError::AddressDeniedForCoin {
            address: env.env.sender,
            coin_type: env.coin_type(),
        },
    );
    env.assert_kept_at_commit(&transfer_tx, 5);

    let effects = env.execute_at(transfer_tx, 5).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Success,
        "execution must not check the sender against the inputs"
    );
    assert!(
        effects
            .unchanged_shared_objects()
            .contains(&(ObjectId::DENY_LIST, UnchangedSharedKind::PerEpochConfig)),
        "the check of the written coin's owner must record the deny list as read"
    );
}

// Without the P-COOL flow, as on Mainnet and Testnet, execution checks the
// coin deny list only for the owners of the coins it writes, so a sender denied
// in the previous epoch and allowed again in this one has its transfer
// executed.
#[tokio::test]
async fn test_execution_does_not_check_sender_without_the_pcool_flow() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(false);
        config
    });
    let env = RegulatedCoinEnv::new().await;
    env.deny_sender().await;
    // The epoch change carries the config over and installs its own override.
    drop(guard);
    env.reconfigure().await;
    env.undeny_sender().await;

    let transfer_tx = env.build_transfer().await;
    let effects = env.execute(transfer_tx).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Success,
        "execution must not check the sender without the P-COOL flow"
    );
}

// Without the P-COOL flow, as on Mainnet and Testnet, execution checks the coin
// deny list for the owners of the coins it writes, against the deny list as settled before
// this epoch, and records the deny list as read. A transfer to an address
// denied in the previous epoch fails, charging gas. A transfer to an address
// that is not denied succeeds and changes only the coin and the gas object.
#[tokio::test]
async fn test_regulated_coin_transfer_effects_without_the_pcool_flow() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(false);
        config
    });
    let env = RegulatedCoinEnv::new().await;
    let denied_recipient = dbg_addr(3);
    env.call_deny_list("deny_list_v1_add", Some(denied_recipient), None)
        .await;
    // The epoch change carries the config over and installs its own override.
    drop(guard);
    env.reconfigure().await;

    let effects = env
        .execute(env.build_transfer_to(denied_recipient).await)
        .await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::AddressDeniedForCoin {
                address: denied_recipient,
                coin_type: env.coin_type(),
            },
            command: None,
        }
    );
    assert!(effects.gas_cost_summary().gas_used() > 0);
    assert_eq!(
        effects.unchanged_shared_objects(),
        vec![(ObjectId::DENY_LIST, UnchangedSharedKind::PerEpochConfig)]
    );

    let effects = env.execute(env.build_transfer_to(dbg_addr(2)).await).await;
    assert_eq!(effects.status(), &ExecutionStatus::Success);
    let mut changed: Vec<ObjectId> = effects
        .all_changed_objects()
        .iter()
        .map(|(changed, _)| *changed.reference().object_id())
        .collect();
    changed.sort();
    let mut expected = vec![env.coin_id, *effects.gas_object().reference().object_id()];
    expected.sort();
    assert_eq!(changed, expected, "only the coin and the gas object change");
    assert_eq!(
        effects.unchanged_shared_objects(),
        vec![(ObjectId::DENY_LIST, UnchangedSharedKind::PerEpochConfig)],
        "the deny list is the only shared object read"
    );
}

// Under P-COOL, a transaction spending a coin whose global pause was settled at
// the epoch boundary and lifted at commit 1 of this epoch is admitted, dropped
// at commit 4 (horizon 0), and kept at commit 5 (horizon 1). Execution still
// fails it, charging gas: the check of the coins it writes reads the pause as
// settled before this epoch.
#[tokio::test]
async fn test_tx_spending_coin_unpaused_this_epoch_is_kept_once_the_horizon_reaches_it() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    env.pause().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;
    env.unpause_at(1).await;

    let transfer_tx = env.build_transfer().await;
    env.validation_check(&transfer_tx)
        .await
        .expect("admission must accept a coin the latest deny list does not pause");
    env.assert_dropped_at_commit(
        &transfer_tx,
        4,
        UserInputError::CoinTypeGlobalPause {
            coin_type: env.coin_type(),
        },
    );
    env.assert_kept_at_commit(&transfer_tx, 5);

    let effects = env.execute_at(transfer_tx, 5).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::CoinTypeGlobalPause {
                coin_type: env.coin_type(),
            },
            command: None,
        },
        "the check of the written coin must fail the kept transaction for the pause settled \
         before this epoch"
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
}

// Under P-COOL, a global pause enabled at commit 1 of this epoch is checked as
// of the horizon: at commit 4 (horizon 0) it is not visible yet, so a transfer
// of the paused coin is kept; at commit 5 (horizon 1) it is dropped.
#[tokio::test]
async fn test_tx_spending_coin_paused_this_epoch_is_dropped_once_the_horizon_reaches_it() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;

    let transfer_tx = env.build_transfer().await;
    env.pause_at(1).await;
    env.assert_kept_at_commit(&transfer_tx, 4);
    env.assert_dropped_at_commit(
        &transfer_tx,
        5,
        UserInputError::CoinTypeGlobalPause {
            coin_type: env.coin_type(),
        },
    );
}

// Under P-COOL, the first denial of a coin type creates its deny-list config. A
// sender denied at commit 1 is checked against the deny list as of the horizon:
// at commit 4 (horizon 0) the config does not exist yet, before or after this
// validator executed the denial, so the transfer is kept; at commit 5 (horizon
// 1) it is dropped.
#[tokio::test]
async fn test_sender_denied_with_a_new_config_is_dropped_once_the_horizon_reaches_it() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;

    let transfer_tx = env.build_transfer().await;
    env.assert_kept_at_commit(&transfer_tx, 4);

    env.deny_sender_at(1).await;
    assert!(
        get_per_type_coin_deny_list_v1(&env.coin_type(), &env.env.authority.get_object_store())
            .is_some(),
        "the denial must create the coin type's config"
    );
    env.assert_kept_at_commit(&transfer_tx, 4);
    env.assert_dropped_at_commit(
        &transfer_tx,
        5,
        UserInputError::AddressDeniedForCoin {
            address: env.env.sender,
            coin_type: env.coin_type(),
        },
    );
}

// Under P-COOL, the sender is checked against the coin types of its receiving
// objects at the commit too, with the deny list as of the horizon, as for its
// inputs. A sender denied at commit 1 is refused at admission, which reads the
// latest deny list. A transaction that receives a coin of that type from an
// object the sender owns and sends it on can still be sequenced, by a faulty
// validator or one that admitted it before executing the denial: at commit 4
// (horizon 0) the denial is not visible yet, so it is kept; at commit 5
// (horizon 1) it is dropped.
#[tokio::test]
async fn test_sender_denied_this_epoch_receiving_a_coin_is_dropped_once_the_horizon_reaches_it() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;

    // The regulated coin is sent to an object the sender owns, to be received
    // from it later.
    let parent_id = env.create_parent().await;
    env.transfer_coin(parent_id.into()).await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;

    env.deny_sender_at(1).await;
    let receive_tx = env.build_receive_and_send(parent_id, dbg_addr(2)).await;
    let err = env.validation_check(&receive_tx).await.unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::AddressDeniedForCoin { address, .. }
            } if *address == env.env.sender
        ),
        "unexpected error: {err:?}"
    );

    env.assert_kept_at_commit(&receive_tx, 4);
    env.assert_dropped_at_commit(
        &receive_tx,
        5,
        UserInputError::AddressDeniedForCoin {
            address: env.env.sender,
            coin_type: env.coin_type(),
        },
    );
}

// Under P-COOL, the sender is checked against its receiving objects at the
// commit, with the deny list as of the horizon, and against the coins the Move
// code receives at execution, with the deny list as the epoch started. A sender
// denied before this epoch and allowed again at commit 1 is admitted, since
// admission reads the latest deny list. A transaction receiving a coin of that
// type and sending it on is dropped at commit 4 (horizon 0) and kept from
// commit 5 (horizon 1) on, and execution still fails it, as the denial holds
// there until the epoch ends.
#[tokio::test]
async fn test_execution_fails_denied_sender_forwarding_a_received_coin() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;

    // The regulated coin is sent to an object the sender owns, to be received
    // from it later.
    let parent_id = env.create_parent().await;
    env.transfer_coin(parent_id.into()).await;

    env.deny_sender().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;
    env.undeny_sender_at(1).await;

    let receive_tx = env.build_receive_and_send(parent_id, dbg_addr(2)).await;
    env.validation_check(&receive_tx)
        .await
        .expect("admission must accept a sender the latest deny list allows");
    env.assert_dropped_at_commit(
        &receive_tx,
        4,
        UserInputError::AddressDeniedForCoin {
            address: env.env.sender,
            coin_type: env.coin_type(),
        },
    );
    env.assert_kept_at_commit(&receive_tx, 5);

    let effects = env.execute_at(receive_tx, 5).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::AddressDeniedForCoin {
                address: env.env.sender,
                coin_type: env.coin_type(),
            },
            command: None,
        },
        "execution must fail the kept transaction for the denial settled before this epoch"
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert_eq!(
        env.env.authority.get_object(&env.coin_id).unwrap().owner,
        Owner::Address(parent_id.into()),
        "the regulated coin must stay with the parent"
    );
}

// Under P-COOL, a transaction receiving a coin whose global pause was settled
// at the epoch boundary and lifted at commit 1 is admitted. It is dropped at
// commit 4 (horizon 0) and kept from commit 5 (horizon 1) on. Execution still
// fails it, charging gas: the coin it sends on is checked against the pause as
// it stood at the start of the epoch.
#[tokio::test]
async fn test_execution_fails_tx_receiving_coin_unpaused_this_epoch() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    let parent_id = env.create_parent().await;
    env.transfer_coin(parent_id.into()).await;

    env.pause().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;
    env.unpause_at(1).await;

    let receive_tx = env.build_receive_and_send(parent_id, dbg_addr(2)).await;
    env.validation_check(&receive_tx)
        .await
        .expect("admission must accept a coin the latest deny list does not pause");
    env.assert_dropped_at_commit(
        &receive_tx,
        4,
        UserInputError::CoinTypeGlobalPause {
            coin_type: env.coin_type(),
        },
    );
    env.assert_kept_at_commit(&receive_tx, 5);

    let effects = env.execute_at(receive_tx, 5).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::CoinTypeGlobalPause {
                coin_type: env.coin_type(),
            },
            command: None,
        },
        "execution must fail the kept transaction for the pause settled before this epoch"
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
}

// Under P-COOL, a sender that is not denied for a coin type receives a coin of
// that type and sends it on, and execution does not fail the transaction.
#[tokio::test]
async fn test_execution_lets_sender_forward_a_received_coin() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;

    // The regulated coin is sent to an object the sender owns, to be received
    // from it later.
    let parent_id = env.create_parent().await;
    env.transfer_coin(parent_id.into()).await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;

    let receive_tx = env.build_receive_and_send(parent_id, dbg_addr(2)).await;
    let effects = env.execute_at(receive_tx, 1).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Success,
        "execution must not fail a sender that is not denied"
    );
    assert_eq!(
        env.env.authority.get_object(&env.coin_id).unwrap().owner,
        Owner::Address(dbg_addr(2)),
        "the sender forwarded the regulated coin"
    );
}

// Under P-COOL, the sender is not checked for a received IOTA coin. A sender
// denied for the regulated coin type receives an IOTA coin and sends it on.
// Execution succeeds, and the effects do not record the deny list as read.
#[tokio::test]
async fn test_execution_does_not_check_sender_for_a_received_iota_coin() {
    let epoch_zero = RegulatedCoinEnv::epoch_zero_override();
    let env = RegulatedCoinEnv::new().await;
    let parent_id = env.create_parent().await;

    let iota_coin = Object::with_owner_for_testing(env.env.sender);
    env.env.authority.insert_genesis_object(iota_coin.clone());
    let send_tx = TestTransactionBuilder::new(env.env.sender, env.fresh_gas_ref(), env.rgp())
        .transfer(iota_coin.object_ref(), parent_id.into())
        .build_and_sign(&env.env.private_key);
    let effects = env.execute(send_tx).await;
    assert!(effects.status().is_success(), "{:?}", effects.status());

    env.deny_sender().await;
    drop(epoch_zero);
    let _guard = env.reconfigure_with_the_pcool_flow().await;

    let receive_tx = TestTransactionBuilder::new(env.env.sender, env.fresh_gas_ref(), env.rgp())
        .move_call(
            env.package_id,
            "receiver",
            "receive_and_send",
            vec![
                CallArg::ImmutableOrOwned(env.env.get_latest_object_ref(&parent_id).await),
                CallArg::Receiving(env.env.get_latest_object_ref(&iota_coin.id()).await),
                CallArg::pure(&dbg_addr(2)),
            ],
        )
        .with_type_args(vec![TypeTag::Struct(Box::new(StructTag::new_gas()))])
        .build_and_sign(&env.env.private_key);
    let effects = env.execute_at(receive_tx, 1).await;
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Success,
        "execution must not check the sender for an IOTA coin"
    );
    assert_eq!(
        env.env.authority.get_object(&iota_coin.id()).unwrap().owner,
        Owner::Address(dbg_addr(2)),
        "the denied sender forwarded the IOTA coin"
    );
    assert!(
        !effects
            .unchanged_shared_objects()
            .contains(&(ObjectId::DENY_LIST, UnchangedSharedKind::PerEpochConfig)),
        "no regulated coin type was looked up"
    );
}

/// A published `coin_deny_list_v1_mintable` package - the regulated coin
/// type, its deny cap, and one minted coin - with helpers for the deny-list
/// operations the tests exercise.
struct RegulatedCoinEnv {
    env: TestEnv,
    package_id: ObjectId,
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
                .get_object(&created.reference().object_id)
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

        let package_id = package_id.expect("package must be created");
        let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
            package_id,
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
            package_id,
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

    async fn reconfigure(&self) {
        self.env.authority.reconfigure_for_testing().await;
    }

    /// Executes a `0x2::coin` deny-list function over the regulated coin
    /// type; `deny_list_v1_add`/`deny_list_v1_remove` take a target address,
    /// the global pause functions take none. `commit` is the consensus commit
    /// the call is assigned to, which every execution with
    /// `enable_pcool_flow` on needs.
    async fn call_deny_list(
        &self,
        function: &'static str,
        address: Option<Address>,
        commit: Option<CommitIndex>,
    ) {
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
        if let Some(commit) = commit {
            self.assign_commit(&tx, commit);
        }

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
        self.call_deny_list("deny_list_v1_add", Some(self.env.sender), None)
            .await;
    }

    async fn undeny_sender(&self) {
        self.call_deny_list("deny_list_v1_remove", Some(self.env.sender), None)
            .await;
    }

    /// [`Self::deny_sender`] assigned to consensus commit `commit`.
    async fn deny_sender_at(&self, commit: CommitIndex) {
        self.call_deny_list("deny_list_v1_add", Some(self.env.sender), Some(commit))
            .await;
    }

    /// [`Self::undeny_sender`] assigned to consensus commit `commit`.
    async fn undeny_sender_at(&self, commit: CommitIndex) {
        self.call_deny_list("deny_list_v1_remove", Some(self.env.sender), Some(commit))
            .await;
    }

    async fn pause(&self) {
        self.call_deny_list("deny_list_v1_enable_global_pause", None, None)
            .await;
    }

    /// [`Self::pause`] assigned to consensus commit `commit`.
    async fn pause_at(&self, commit: CommitIndex) {
        self.call_deny_list("deny_list_v1_enable_global_pause", None, Some(commit))
            .await;
    }

    /// Lifts the global pause in a call assigned to consensus commit `commit`.
    async fn unpause_at(&self, commit: CommitIndex) {
        self.call_deny_list("deny_list_v1_disable_global_pause", None, Some(commit))
            .await;
    }

    /// A transfer of the regulated coin with its own freshly inserted gas
    /// object, so the deny-list calls above (which spend the publisher's gas)
    /// never invalidate its input references.
    async fn build_transfer(&self) -> TransactionEnvelope {
        self.build_transfer_to(dbg_addr(2)).await
    }

    /// [`Self::build_transfer`] to `recipient`.
    async fn build_transfer_to(&self, recipient: Address) -> TransactionEnvelope {
        TestTransactionBuilder::new(self.env.sender, self.fresh_gas_ref(), self.rgp())
            .transfer(
                self.env.get_latest_object_ref(&self.coin_id).await,
                recipient,
            )
            .build_and_sign(&self.env.private_key)
    }

    /// Executes a transfer of the regulated coin to `recipient`.
    async fn transfer_coin(&self, recipient: Address) {
        let effects = self.execute(self.build_transfer_to(recipient).await).await;
        assert!(effects.status().is_success(), "{:?}", effects.status());
    }

    /// Creates a `receiver::Parent` owned by the sender and returns its id.
    async fn create_parent(&self) -> ObjectId {
        let tx = TestTransactionBuilder::new(self.env.sender, self.fresh_gas_ref(), self.rgp())
            .move_call(self.package_id, "receiver", "create", vec![])
            .build_and_sign(&self.env.private_key);
        let effects = self.execute(tx).await;
        assert!(effects.status().is_success(), "{:?}", effects.status());
        let created = effects.created();
        assert_eq!(created.len(), 1, "{created:?}");
        created[0].reference().object_id
    }

    /// A `receiver::receive_and_send` call: the sender receives the regulated
    /// coin from `parent_id` and sends it on to `to`.
    async fn build_receive_and_send(
        &self,
        parent_id: ObjectId,
        to: Address,
    ) -> TransactionEnvelope {
        TestTransactionBuilder::new(self.env.sender, self.fresh_gas_ref(), self.rgp())
            .move_call(
                self.package_id,
                "receiver",
                "receive_and_send",
                vec![
                    CallArg::ImmutableOrOwned(self.env.get_latest_object_ref(&parent_id).await),
                    CallArg::Receiving(self.env.get_latest_object_ref(&self.coin_id).await),
                    CallArg::pure(&to),
                ],
            )
            .with_type_args(vec![self.regulated_coin_type.clone()])
            .build_and_sign(&self.env.private_key)
    }

    /// A gas object freshly inserted for the sender, so the deny-list calls
    /// (which spend the publisher's gas) never invalidate the references of
    /// the transaction it pays for.
    fn fresh_gas_ref(&self) -> ObjectReference {
        let gas_object = Object::with_owner_for_testing(self.env.sender);
        self.env.authority.insert_genesis_object(gas_object.clone());
        gas_object.object_ref()
    }

    /// Runs the admission checks, `handle_transaction_validation_checks`. The
    /// epoch store is fetched per call, so the check follows epoch changes.
    async fn validation_check(
        &self,
        transaction: &TransactionEnvelope,
    ) -> IotaResult<Vec<ObjectReference>> {
        let epoch_store = self.env.authority.epoch_store_for_testing();

        self.env
            .authority
            .handle_transaction_validation_checks(
                &VerifiedTransaction::new_unchecked(transaction.clone()),
                &epoch_store,
                &self.env.authority.config.transaction_deny_config,
            )
            .await
    }

    /// The regulated coin type as the deny list and the statuses name it.
    fn coin_type(&self) -> String {
        self.regulated_coin_type.to_canonical_string(false)
    }

    /// Post-consensus validation of the transaction at consensus commit
    /// `commit`, the path with `enable_pcool_flow` on.
    fn validate_at_commit(
        &self,
        transaction: &TransactionEnvelope,
        commit: CommitIndex,
    ) -> IotaResult<ValidationAtCommit> {
        let epoch_store = self.env.authority.epoch_store_for_testing();
        let reader = CommitIndexedReader::new(
            self.env.authority.get_object_cache_reader().clone(),
            (*epoch_store).clone(),
            commit,
        );
        self.env
            .authority
            .handle_transaction_validation_checks_at_commit(
                &reader,
                &VerifiedTransaction::new_unchecked(transaction.clone()),
                &epoch_store,
                &self.env.authority.config.transaction_deny_config,
                VerifierLimitsSource::ProtocolConfig,
            )
    }

    /// Asserts that post-consensus validation at consensus commit `commit`
    /// keeps the transaction.
    fn assert_kept_at_commit(&self, transaction: &TransactionEnvelope, commit: CommitIndex) {
        let verdict = self.validate_at_commit(transaction, commit).unwrap();
        assert!(
            matches!(verdict, ValidationAtCommit::Keep(_)),
            "post-consensus validation at commit {commit} must keep the transaction, got \
             {verdict:?}"
        );
    }

    /// Asserts that post-consensus validation at consensus commit `commit`
    /// drops the transaction with `error`.
    fn assert_dropped_at_commit(
        &self,
        transaction: &TransactionEnvelope,
        commit: CommitIndex,
        error: UserInputError,
    ) {
        let result = self.validate_at_commit(transaction, commit);
        assert!(
            matches!(&result, Err(IotaError::UserInput { error: got }) if *got == error),
            "post-consensus validation at commit {commit} must drop the transaction with \
             {error:?}, got {result:?}"
        );
    }

    /// Executes the transaction after the checks a validator runs before
    /// signing it, and returns its effects.
    async fn execute(&self, transaction: TransactionEnvelope) -> TransactionEffects {
        let (_, effects, _) = send_and_confirm_transaction_with_execution_error(
            &self.env.authority,
            None,
            transaction,
            false,
            false,
        )
        .await
        .unwrap();
        effects.into_data()
    }

    /// [`Self::execute`] with the transaction assigned to consensus commit
    /// `commit` first, as every execution with `enable_pcool_flow`
    /// on needs.
    async fn execute_at(
        &self,
        transaction: TransactionEnvelope,
        commit: CommitIndex,
    ) -> TransactionEffects {
        self.assign_commit(&transaction, commit);
        self.execute(transaction).await
    }

    /// Assigns `transaction` to consensus commit `commit`. With
    /// `enable_pcool_flow` on, the bookkeeping stops the node on
    /// an execution the consensus handler has not assigned to a commit, so
    /// that no object becomes readable without its row.
    fn assign_commit(&self, transaction: &TransactionEnvelope, commit: CommitIndex) {
        self.env
            .authority
            .epoch_store_for_testing()
            .assign_commit_to_transactions(
                commit,
                vec![TransactionKey::Digest(*transaction.digest())],
            );
    }

    /// The protocol config override for epoch 0 of a test that turns the
    /// P-COOL flow on at the epoch change. The setup executes outside any
    /// consensus commit, which the flow's bookkeeping refuses.
    fn epoch_zero_override() -> OverrideGuard {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(false);
            config
        })
    }

    /// Advances to epoch 1 with `enable_pcool_flow` on, which
    /// settles the deny-list entries written in epoch 0, and returns the
    /// override that keeps the flag on for the rest of the test. The caller
    /// drops the epoch-0 override first: the epoch change installs its own
    /// and refuses to run while another one is installed.
    async fn reconfigure_with_the_pcool_flow(&self) -> OverrideGuard {
        let mut epoch_one_config = self
            .env
            .authority
            .epoch_store_for_testing()
            .protocol_config()
            .clone();
        epoch_one_config.set_enable_pcool_flow_for_testing(true);
        self.env
            .authority
            .reconfigure_for_testing_with_protocol_config(epoch_one_config)
            .await;
        let epoch_store = self.env.authority.epoch_store_for_testing();
        assert_eq!(epoch_store.epoch(), 1);
        assert!(epoch_store.pcool_bookkeeping_enabled());
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config
        })
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
