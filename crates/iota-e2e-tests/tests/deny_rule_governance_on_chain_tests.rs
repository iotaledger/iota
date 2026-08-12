// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end battery for on-chain deny-rule governance: the
//! `TransactionDenyRules` object lifecycle, delta injection at commit
//! boundaries, and their interaction with checkpoints, reconfiguration, and
//! node faults.
//!
//! The protocol config override enabling the feature flags is thread-local,
//! so these tests only work under the simulator, where all nodes share one
//! thread: `cargo simtest -p iota-e2e-tests deny_rule`.

#![cfg(msim)]

use std::time::Duration;

use iota_config::transaction_deny_config::TransactionDenyConfigBuilder;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, DenyRuleSet};
use iota_types::{
    IOTA_TRANSACTION_DENY_RULES_OBJECT_ID, transaction_deny_rules::get_transaction_deny_rules,
};
use test_cluster::{TestCluster, TestClusterBuilder};

/// Enables both governance flags and the two injection knobs. The returned
/// guard must stay alive for the whole test.
fn governance_overrides(
    max_entries_per_tx: u64,
    removal_grace_round_floor: u64,
) -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_deny_rule_governance_for_testing(true);
        config.set_deny_rule_governance_on_chain_for_testing(true);
        config.set_deny_rule_update_max_entries_per_tx_for_testing(max_entries_per_tx);
        config.set_deny_rule_removal_grace_round_floor_for_testing(removal_grace_round_floor);
        config
    })
}

/// The deny-rule state read back by walking the on-chain object on this
/// node, or `None` while the object does not exist.
fn walked_rules(handle: &iota_node::IotaNodeHandle) -> Option<DenyRuleSet> {
    handle.with(|node| {
        get_transaction_deny_rules(node.state().get_object_store().as_ref())
            .expect("walking the deny-rules object must succeed")
    })
}

/// Polls until `condition` holds on every validator, panicking after ~60s.
/// Every poll also asserts that no validator reported a failed update
/// execution — an invariant violation no test expects, better surfaced
/// directly than as a timeout.
async fn poll_all_validators(
    test_cluster: &TestCluster,
    what: &str,
    condition: impl Fn(&iota_node::IotaNodeHandle) -> bool,
) {
    for _ in 0..120 {
        let handles = test_cluster.swarm.validator_node_handles();
        for handle in &handles {
            let failures = handle.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .metrics_for_testing()
                    .deny_rule_update_execution_failures
                    .get()
            });
            assert_eq!(failures, 0, "a deny-rule update failed execution");
        }
        if handles.iter().all(&condition) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("timed out waiting for {what} on all validators");
}

/// Polls until `condition` holds on the fullnode, panicking after ~60s. The
/// fullnode follows checkpoints and lags the validators by a cycle, so any
/// assertion over its state or its checkpoints has to wait for it.
async fn poll_fullnode(
    test_cluster: &TestCluster,
    what: &str,
    condition: impl Fn(&iota_node::IotaNodeHandle) -> bool,
) {
    for _ in 0..120 {
        if condition(&test_cluster.fullnode_handle.iota_node) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("timed out waiting for {what} on the fullnode");
}

/// Restarts a validator with a replaced local deny config; the restarted
/// node re-announces it with a fresh generation.
async fn restart_with_deny_config(
    test_cluster: &TestCluster,
    name: &iota_types::base_types::AuthorityName,
    config: iota_config::transaction_deny_config::TransactionDenyConfig,
) {
    test_cluster.stop_node(name);
    // The stopped node releases its store asynchronously; reopening too
    // early trips the RocksDB lock.
    tokio::time::sleep(Duration::from_secs(10)).await;
    test_cluster
        .swarm
        .node(name)
        .unwrap()
        .config()
        .transaction_deny_config = config;
    test_cluster.start_node(name).await;
}

/// The digests of all `TransactionDenyRulesUpdate` transactions found in
/// this node's executed checkpoints, in checkpoint order.
fn checkpointed_update_digests(
    handle: &iota_node::IotaNodeHandle,
) -> Vec<iota_sdk_types::TransactionDigest> {
    use iota_sdk_types::TransactionKind;
    use iota_types::{messages_checkpoint::CheckpointContentsExt, transaction::TransactionAPI};

    handle.with(|node| {
        let state = node.state();
        let checkpoint_store = state.checkpoint_store.clone();
        let cache = state.get_transaction_cache_reader();
        let highest = checkpoint_store
            .get_highest_executed_checkpoint()
            .unwrap()
            .expect("checkpoints must exist")
            .sequence_number();

        let mut digests = Vec::new();
        for sequence in 0..=highest {
            let Some(checkpoint) = checkpoint_store
                .get_checkpoint_by_sequence_number(sequence)
                .unwrap()
            else {
                continue;
            };
            let Some(contents) = checkpoint_store
                .get_checkpoint_contents(&checkpoint.data().contents_digest)
                .unwrap()
            else {
                continue;
            };
            for execution_digests in contents.iter() {
                let Some(transaction) = cache
                    .try_get_transaction_block(&execution_digests.transaction)
                    .unwrap()
                else {
                    continue;
                };
                if matches!(
                    transaction.data().transaction().kind(),
                    TransactionKind::TransactionDenyRulesUpdate(_)
                ) {
                    digests.push(execution_digests.transaction);
                }
            }
        }
        digests
    })
}

/// Adds `new_validator` as a candidate, stakes the joining minimum, and
/// requests addition; the validator activates at the next epoch change and
/// enters the committee at the one after.
async fn execute_join_transactions(
    test_cluster: &TestCluster,
    new_validator: &iota_swarm_config::genesis_config::ValidatorGenesisConfig,
) {
    use std::collections::BTreeSet;

    use iota_test_transaction_builder::TestTransactionBuilder;

    let address = new_validator.account_key_pair.public_key().derive_address();
    let rgp = test_cluster.get_reference_gas_price().await;

    let gas = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(address)
        .await
        .unwrap()
        .expect("the candidate address is funded at genesis");
    let tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_request_add_validator_candidate(
            &new_validator.to_validator_info_with_random_name().into(),
        )
        .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(tx).await;

    let min_stake = test_cluster.protocol_config().min_validator_joining_stake();
    let stake_coin = test_cluster
        .wallet
        .gas_for_owner_budget(address, min_stake, Default::default())
        .await
        .unwrap()
        .1
        .object_ref();
    let gas = test_cluster
        .wallet
        .gas_for_owner_budget(address, 0, BTreeSet::from([stake_coin.object_id]))
        .await
        .unwrap()
        .1
        .object_ref();
    let stake_tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_staking(stake_coin, address)
        .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(stake_tx).await;

    let gas = test_cluster
        .wallet
        .get_object_ref(gas.object_id)
        .await
        .unwrap();
    let tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_request_add_validator()
        .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(tx).await;
}

/// Stops a validator, deletes the given store directories, and restarts it.
async fn wipe_and_restart(
    test_cluster: &TestCluster,
    name: &iota_types::base_types::AuthorityName,
    paths: Vec<std::path::PathBuf>,
) {
    test_cluster.stop_node(name);
    tokio::time::sleep(Duration::from_secs(10)).await;
    for path in paths {
        if path.exists() {
            std::fs::remove_dir_all(&path).unwrap();
        }
    }
    test_cluster.start_node(name).await;
}

/// Injects a rule, wipes the given directories on one validator, and asserts
/// the restarted node reconverges: its object matches its peers, and the
/// next epoch boundary passes — a diverged mirror would abort the simulation
/// through the boundary guard.
async fn wiped_validator_reconverges(
    wiped_paths: impl Fn(&std::path::Path, &std::path::Path) -> Vec<std::path::PathBuf>,
) -> (TestCluster, iota_protocol_config::OverrideGuard) {
    let guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    let victim = test_cluster.get_validator_pubkeys()[0];
    let paths = {
        let node = test_cluster.swarm.node(&victim).unwrap();
        let config = node.config();
        let consensus_db = config
            .consensus_config
            .as_ref()
            .expect("validators run consensus")
            .db_path()
            .to_path_buf();
        wiped_paths(&config.db_path(), &consensus_db)
    };
    wipe_and_restart(&test_cluster, &victim, paths).await;

    // The wiped node must reconverge on the object...
    let victim_handle = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .expect("the node restarted");
    for _ in 0..240 {
        if walked_rules(&victim_handle)
            .is_some_and(|walked| walked.denied_addresses.contains(&denied))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(
        walked_rules(&victim_handle)
            .is_some_and(|walked| walked.denied_addresses.contains(&denied)),
        "the wiped validator did not reconverge on the object"
    );

    // ...and cross the next boundary: a mirror that diverged from the object
    // would abort reconfiguration in the simulator.
    test_cluster.force_new_epoch().await;
    let reference = walked_rules(&test_cluster.swarm.validator_node_handles()[0]).unwrap();
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), reference);
        handle.with(|node| {
            assert_eq!(
                *node
                    .state()
                    .epoch_store_for_testing()
                    .get_mirrored_transaction_deny_rules(),
                reference,
                "every mirror must seed from the same object after the boundary"
            );
        });
    }
    (test_cluster, guard)
}

/// A validator whose stores are fully wiped rejoins through state sync and
/// reconverges on the object and the mirror.
#[sim_test]
async fn deny_rule_full_wipe_resyncs_through_state_sync() {
    telemetry_subscribers::init_for_testing();
    let (_cluster, _guard) = wiped_validator_reconverges(|db_path, consensus_db| {
        vec![
            db_path.join("store"),
            db_path.join("epochs"),
            db_path.join("checkpoints"),
            consensus_db.to_path_buf(),
        ]
    })
    .await;
}

/// A wiped epoch database rewinds the replay cutoff to zero: the whole epoch
/// replays from the intact consensus store and the mirror reconverges from
/// the epoch-start seed. The deny-rule replay itself is deterministic
/// (identical chunk digests), but node recovery deadlocks whenever built
/// checkpoints ran ahead of executed ones at shutdown: shared-version
/// re-initialization from mid-epoch object state shifts the replayed
/// assignments and starves the unexecuted tail behind the checkpoint-rebuild
/// startup barrier.
#[sim_test]
#[ignore = "https://github.com/iotaledger/iota/issues/12632"]
async fn deny_rule_epoch_db_wipe_replays_and_reconverges() {
    telemetry_subscribers::init_for_testing();
    let (test_cluster, _guard) = wiped_validator_reconverges(|db_path, _| {
        let store = db_path.join("store");
        std::fs::read_dir(&store)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("epoch_"))
                    .then_some(path)
            })
            .collect()
    })
    .await;

    // The recovered validator keeps deriving deltas like its peers: a second
    // rule activated after the wipe reaches the object everywhere.
    let second = Address::new([43u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(Address::new([42u8; 32]))
        .add_denied_address(second)
        .build();
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    poll_all_validators(&test_cluster, "the post-wipe delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&second))
    })
    .await;
}

/// The object does not exist while the epoch that enabled the flags is still
/// running; the first epoch boundary creates it, and every node seeds its
/// mirror and enforcement from the walked (empty) state.
#[sim_test]
async fn deny_rule_object_is_created_at_the_first_epoch_boundary() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    assert!(
        test_cluster
            .get_object_from_fullnode_store(&IOTA_TRANSACTION_DENY_RULES_OBJECT_ID)
            .await
            .is_none(),
        "the object must not exist before the first boundary"
    );

    test_cluster.force_new_epoch().await;

    for handle in test_cluster.swarm.validator_node_handles() {
        let walked = walked_rules(&handle)
            .expect("the object must exist on every validator after the boundary");
        assert_eq!(walked, DenyRuleSet::default());
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            assert_eq!(
                *store.get_mirrored_transaction_deny_rules(),
                DenyRuleSet::default()
            );
        });
    }
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&IOTA_TRANSACTION_DENY_RULES_OBJECT_ID)
            .await
            .is_some(),
        "the fullnode must also hold the created object"
    );
}

/// An activated rule reaches the on-chain object: the local configs
/// aggregate to an active set, the first commits of the epoch after creation
/// inject the delta, and every node's walked object state converges to it,
/// with the mirror in lockstep.
#[sim_test]
async fn deny_rule_activations_are_injected_into_the_object() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    // Epoch 0 activates the rule in memory; the boundary creates the object;
    // the first commits of epoch 1 inject the delta.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    for handle in test_cluster.swarm.validator_node_handles() {
        let walked = walked_rules(&handle).unwrap();
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            assert_eq!(
                *store.get_mirrored_transaction_deny_rules(),
                walked,
                "the mirror must match the object once injections execute"
            );
            assert!(
                store
                    .get_active_transaction_deny_rules()
                    .denied_addresses
                    .contains(&denied)
            );
        });
    }
}

/// A delta larger than `deny_rule_update_max_entries_per_tx` splits into
/// multiple update transactions in the same commit, and the object still
/// converges to the full aggregate.
#[sim_test]
async fn deny_rule_chunked_delta_converges_across_multiple_transactions() {
    telemetry_subscribers::init_for_testing();
    // 7 entries at 3 per transaction: three chunks in one commit.
    let _guard = governance_overrides(3, 0);

    let denied: Vec<Address> = (0..7u8).map(|i| Address::new([i + 1; 32])).collect();
    let mut builder = TransactionDenyConfigBuilder::new();
    for address in &denied {
        builder = builder.add_denied_address(*address);
    }
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(builder.build())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the chunked delta", |handle| {
        walked_rules(handle).is_some_and(|walked| {
            denied
                .iter()
                .all(|address| walked.denied_addresses.contains(address))
        })
    })
    .await;

    // The mirror reached the same target on every validator.
    for handle in test_cluster.swarm.validator_node_handles() {
        handle.with(|node| {
            let mirror = node
                .state()
                .epoch_store_for_testing()
                .get_mirrored_transaction_deny_rules();
            assert_eq!(mirror.denied_addresses.len(), denied.len());
        });
    }
}

/// While the grace round floor has not passed, a withdrawn rule stays on the
/// object and stays enforced: removals are held, additions were immediate.
/// The companion withdrawal test proves the same machinery does remove the
/// entry once the grace allows it.
#[sim_test]
async fn deny_rule_removals_are_held_while_the_grace_lasts() {
    telemetry_subscribers::init_for_testing();
    // A floor no in-test commit round reaches: removals stay locked.
    let _guard = governance_overrides(1000, 100_000);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    // Withdraw the rule on three of four validators: support falls below
    // f+1, so the aggregate drops the entry.
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }

    // Rounds keep ticking well past the withdrawal; the entry must survive
    // on the object, in the mirror, and in enforcement.
    tokio::time::sleep(Duration::from_secs(10)).await;
    for handle in test_cluster.swarm.validator_node_handles() {
        assert!(
            walked_rules(&handle)
                .unwrap()
                .denied_addresses
                .contains(&denied),
            "a removal must not reach the object during the grace"
        );
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            assert!(
                store
                    .get_mirrored_transaction_deny_rules()
                    .denied_addresses
                    .contains(&denied)
            );
            assert!(
                store
                    .get_active_transaction_deny_rules()
                    .denied_addresses
                    .contains(&denied),
                "enforcement must not lapse while the removal is held"
            );
        });
    }
}

/// With the grace floor at zero, a withdrawal removes the entry from the
/// object. Re-adding the rule re-injects it, as happens when a late
/// supporter announces after the removal.
#[sim_test]
async fn deny_rule_withdrawal_and_re_add_converges() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config.clone())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    // Withdraw on three of four validators: support falls below f+1 and the
    // unlocked removal must reach the object everywhere.
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }
    poll_all_validators(&test_cluster, "the injected removal", |handle| {
        walked_rules(handle).is_some_and(|walked| !walked.denied_addresses.contains(&denied))
    })
    .await;

    // Re-add: the second half re-injects the addition immediately.
    for name in names.iter().take(3) {
        restart_with_deny_config(&test_cluster, name, deny_config.clone()).await;
    }
    poll_all_validators(&test_cluster, "the re-injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;
}

/// Enforcement survives the epoch boundary through the object alone: once a
/// rule is on chain, an epoch that starts with no local config supporting it
/// (and removals still grace-locked) seeds enforcement from the walked object
/// and keeps rejecting the denied sender from its first commits.
#[sim_test]
async fn deny_rule_enforcement_survives_the_epoch_boundary() {
    use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_GAS_AMOUNT};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::crypto::get_account_key_pair;

    telemetry_subscribers::init_for_testing();
    // A floor no in-test commit round reaches: the withdrawn rule cannot be
    // removed from the object.
    let _guard = governance_overrides(1000, 100_000);

    let (denied, denied_key) = get_account_key_pair();
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_accounts(vec![
            AccountConfig {
                address: Some(denied),
                gas_amounts: vec![DEFAULT_GAS_AMOUNT],
            },
            AccountConfig {
                address: None,
                gas_amounts: vec![DEFAULT_GAS_AMOUNT; 2],
            },
        ])
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    // Epoch 1 injects the rule into the object.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    // Withdraw local support on three of four validators, then cross into
    // an epoch where the aggregate no longer carries the rule.
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }
    test_cluster.force_new_epoch().await;

    // The new epoch's enforcement comes from the walked object.
    for handle in test_cluster.swarm.validator_node_handles() {
        handle.with(|node| {
            assert!(
                node.state()
                    .epoch_store_for_testing()
                    .get_active_transaction_deny_rules()
                    .denied_addresses
                    .contains(&denied),
                "the on-chain rule must stay enforced after the boundary"
            );
        });
    }

    let rgp = test_cluster.get_reference_gas_price().await;
    let denied_gas = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(denied)
        .await
        .unwrap()
        .expect("denied account should own gas");
    let denied_tx = TestTransactionBuilder::new(denied, denied_gas, rgp)
        .transfer_iota(Some(1_000), test_cluster.get_address_0())
        .build_and_sign(&denied_key);
    let error = test_cluster
        .wallet
        .execute_transaction_may_fail(denied_tx)
        .await
        .expect_err("the denied sender must stay rejected after the boundary")
        .to_string();
    assert!(
        error.contains("temporarily disabled"),
        "unexpected rejection reason: {error}"
    );

    // A non-denied sender still executes normally.
    let sender = test_cluster.get_address_0();
    let gas = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(sender)
        .await
        .unwrap()
        .expect("sender should own gas");
    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(Some(1_000), sender)
        .build();
    test_cluster.sign_and_execute_transaction(&tx_data).await;
}

/// A fullnode follows checkpoints only: mid-epoch injections must reach its
/// object, and it must cross the next epoch boundary — where validators
/// compare their mirror against the object and the fullnode holds none.
#[sim_test]
async fn deny_rule_fullnode_converges_and_crosses_the_boundary() {
    use iota_core::authority::epoch_start_configuration::EpochStartConfigTrait;

    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    // Epoch 1: the injected delta must reach the fullnode through checkpoint
    // execution.
    test_cluster.force_new_epoch().await;
    let fullnode = &test_cluster.fullnode_handle.iota_node;
    for _ in 0..120 {
        if fullnode
            .with(|node| {
                get_transaction_deny_rules(node.state().get_object_store().as_ref()).unwrap()
            })
            .is_some_and(|walked| walked.denied_addresses.contains(&denied))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let fullnode_walked = fullnode
        .with(|node| get_transaction_deny_rules(node.state().get_object_store().as_ref()).unwrap())
        .expect("the fullnode must hold the object");
    assert!(fullnode_walked.denied_addresses.contains(&denied));

    // Epoch 2: the boundary must not fire the mirror guard on the fullnode,
    // and its next-epoch seed must equal the walked object.
    test_cluster.force_new_epoch().await;
    fullnode.with(|node| {
        let store = node.state().epoch_store_for_testing();
        assert_eq!(
            store.epoch_start_config().transaction_deny_rules_state(),
            Some(&fullnode_walked),
            "the fullnode's epoch seed must equal the object it executed"
        );
    });
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), fullnode_walked);
    }
}

/// Folding the `TransactionDenyRulesUpdated` event stream reproduces the
/// object: the events are the public audit history, so their deltas and
/// post-update lengths must match the applied state exactly, across both
/// chunked additions and chunked removals.
#[sim_test]
async fn deny_rule_event_fold_matches_the_object() {
    use iota_sdk_types::{ObjectId, TransactionKind};
    use iota_types::{messages_checkpoint::CheckpointContentsExt, transaction::TransactionAPI};

    /// Mirror of the Move event's BCS layout.
    #[derive(serde::Deserialize)]
    struct TransactionDenyRulesUpdatedEvent {
        epoch: u64,
        added_addresses: Vec<Address>,
        removed_addresses: Vec<Address>,
        added_objects: Vec<ObjectId>,
        removed_objects: Vec<ObjectId>,
        added_packages: Vec<ObjectId>,
        removed_packages: Vec<ObjectId>,
        package_publish_disabled: bool,
        package_upgrade_disabled: bool,
        shared_object_disabled: bool,
        user_transaction_disabled: bool,
        receiving_objects_disabled: bool,
        move_authenticator_disabled: bool,
        denied_addresses_len: u64,
        denied_objects_len: u64,
        denied_packages_len: u64,
    }

    telemetry_subscribers::init_for_testing();
    // 7 entries at 3 per transaction: both directions chunk.
    let _guard = governance_overrides(3, 0);

    let denied: Vec<Address> = (0..7u8).map(|i| Address::new([i + 1; 32])).collect();
    let mut builder = TransactionDenyConfigBuilder::new();
    for address in &denied {
        builder = builder.add_denied_address(*address);
    }
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(builder.build())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    // Chunked additions, then chunked removals via withdrawal.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the chunked delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.len() == denied.len())
    })
    .await;
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }
    poll_all_validators(&test_cluster, "the injected removals", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.is_empty())
    })
    .await;
    // The fold below reads the fullnode's checkpoints, which hold every chunk
    // only once the fullnode itself has applied the removals.
    poll_fullnode(&test_cluster, "the injected removals", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.is_empty())
    })
    .await;

    // Collect every update event from the fullnode's checkpoints, in
    // execution order, and fold it.
    let (event_count, fold, walked) = test_cluster.fullnode_handle.iota_node.with(|node| {
        let state = node.state();
        let checkpoint_store = state.checkpoint_store.clone();
        let cache = state.get_transaction_cache_reader();
        let highest = checkpoint_store
            .get_highest_executed_checkpoint()
            .unwrap()
            .expect("checkpoints must exist")
            .sequence_number();

        let mut fold = DenyRuleSet::default();
        let mut event_count = 0usize;
        for sequence in 0..=highest {
            let Some(checkpoint) = checkpoint_store
                .get_checkpoint_by_sequence_number(sequence)
                .unwrap()
            else {
                continue;
            };
            let Some(contents) = checkpoint_store
                .get_checkpoint_contents(&checkpoint.data().contents_digest)
                .unwrap()
            else {
                continue;
            };
            for digests in contents.iter() {
                let Some(transaction) = cache
                    .try_get_transaction_block(&digests.transaction)
                    .unwrap()
                else {
                    continue;
                };
                if !matches!(
                    transaction.data().transaction().kind(),
                    TransactionKind::TransactionDenyRulesUpdate(_)
                ) {
                    continue;
                }
                let events = cache
                    .get_events(&digests.transaction)
                    .expect("an update emits its event");
                for event in &events.0 {
                    assert_eq!(event.type_.name().as_str(), "TransactionDenyRulesUpdated");
                    let event: TransactionDenyRulesUpdatedEvent =
                        bcs::from_bytes(&event.contents).unwrap();
                    event_count += 1;
                    assert_eq!(event.epoch, checkpoint.data().epoch);

                    fold.denied_addresses.extend(event.added_addresses);
                    for address in &event.removed_addresses {
                        fold.denied_addresses.remove(address);
                    }
                    fold.denied_objects.extend(event.added_objects);
                    for id in &event.removed_objects {
                        fold.denied_objects.remove(id);
                    }
                    fold.denied_packages.extend(event.added_packages);
                    for id in &event.removed_packages {
                        fold.denied_packages.remove(id);
                    }
                    fold.package_publish_disabled = event.package_publish_disabled;
                    fold.package_upgrade_disabled = event.package_upgrade_disabled;
                    fold.shared_object_disabled = event.shared_object_disabled;
                    fold.user_transaction_disabled = event.user_transaction_disabled;
                    fold.receiving_objects_disabled = event.receiving_objects_disabled;
                    fold.move_authenticator_disabled = event.move_authenticator_disabled;

                    // The advertised post-update lengths track the fold.
                    assert_eq!(
                        fold.denied_addresses.len() as u64,
                        event.denied_addresses_len
                    );
                    assert_eq!(fold.denied_objects.len() as u64, event.denied_objects_len);
                    assert_eq!(fold.denied_packages.len() as u64, event.denied_packages_len);
                }
            }
        }
        let walked = get_transaction_deny_rules(state.get_object_store().as_ref())
            .unwrap()
            .expect("the object must exist");
        (event_count, fold, walked)
    });

    // 7 additions then 7 removals at 3 entries per transaction: at least
    // three chunks in each direction, and the fold lands on the object.
    assert!(
        event_count >= 6,
        "expected chunked updates in both directions, saw {event_count} events"
    );
    assert_eq!(fold, walked, "the event stream must reproduce the object");
}

/// Every chunk of a split delta is executed in a checkpoint on every
/// validator, and the epoch boundary after the injections passes cleanly —
/// releasing the chunks' shared-version assignments (a leak fails the
/// reconfiguration assert) with `check_all_executed_transactions_in_checkpoint`
/// staying quiet.
#[sim_test]
async fn deny_rule_chunks_are_checkpointed_on_every_validator() {
    telemetry_subscribers::init_for_testing();
    // 7 entries at 3 per transaction: three chunks.
    let _guard = governance_overrides(3, 0);

    let denied: Vec<Address> = (0..7u8).map(|i| Address::new([i + 1; 32])).collect();
    let mut builder = TransactionDenyConfigBuilder::new();
    for address in &denied {
        builder = builder.add_denied_address(*address);
    }
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(builder.build())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the chunked delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.len() == denied.len())
    })
    .await;
    // Give the injected chunks time to reach executed checkpoints everywhere.
    let update_digests = 'digests: {
        for _ in 0..120 {
            let digests = checkpointed_update_digests(&test_cluster.fullnode_handle.iota_node);
            if digests.len() >= 3 {
                break 'digests digests;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("the chunked delta never reached the fullnode's checkpoints");
    };

    for handle in test_cluster.swarm.validator_node_handles() {
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            for digest in &update_digests {
                assert!(
                    store.is_transaction_executed_in_checkpoint(digest).unwrap(),
                    "chunk {digest} must be executed in a checkpoint on every validator"
                );
            }
        });
    }

    // Every validator's certified record carries the identical digest set —
    // the digests embed (epoch, round), so equality also pins the originating
    // commit and the chunk split. A divergent local derivation cannot reach
    // this point: the local-vs-certified checkpoint fork detection is fatal.
    let expected: std::collections::BTreeSet<_> = update_digests.iter().copied().collect();
    for handle in test_cluster.swarm.validator_node_handles() {
        for _ in 0..120 {
            let seen: std::collections::BTreeSet<_> =
                checkpointed_update_digests(&handle).into_iter().collect();
            if seen == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let seen: std::collections::BTreeSet<_> =
            checkpointed_update_digests(&handle).into_iter().collect();
        assert_eq!(
            seen, expected,
            "every validator's checkpoints must carry the identical update digests"
        );
    }

    // The boundary releases the chunks' shared-version assignments and runs
    // the executed-in-checkpoint debug assert.
    test_cluster.force_new_epoch().await;
}

/// A validator restarted mid-epoch with intact stores converges back and
/// keeps deriving new deltas identically after the restart. This test does
/// not control the flush boundary — a fully flushed clean restart satisfies
/// it; the unflushed-tail replay is exercised deterministically by
/// `deny_rule_crash_before_flush_replays_the_derivation`.
#[sim_test]
async fn deny_rule_mid_epoch_restart_re_derives_identically() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config.clone())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;

    // Plain restart with intact stores; the flush boundary is deliberately
    // uncontrolled — the node resumes from its persisted or replayed state.
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    test_cluster.stop_node(&victim);
    tokio::time::sleep(Duration::from_secs(10)).await;
    test_cluster.start_node(&victim).await;

    // A rule activated after the restart must reach the object everywhere,
    // deriving through the restarted validator's recovered mirror.
    let second = Address::new([43u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .add_denied_address(second)
        .build();
    for name in names.iter().skip(1).take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    poll_all_validators(&test_cluster, "the post-restart delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&second))
    })
    .await;

    // The boundary compares every mirror against the object: divergence on
    // the restarted validator would abort the simulation.
    test_cluster.force_new_epoch().await;
    let reference = walked_rules(&test_cluster.swarm.validator_node_handles()[0]).unwrap();
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), reference);
        handle.with(|node| {
            assert_eq!(
                *node
                    .state()
                    .epoch_store_for_testing()
                    .get_mirrored_transaction_deny_rules(),
                reference
            );
        });
    }
}

/// A validator that misses the end of an epoch crosses the boundary by
/// executing synced checkpoints. Its mirror never saw the missed
/// injections. The boundary guard must stay quiet. The rejoined node must
/// derive later deltas like the incumbents.
#[sim_test]
async fn deny_rule_checkpoint_catch_up_crosses_the_boundary_quietly() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    // Create the object before anything is denied.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the created object", |handle| {
        walked_rules(handle).is_some()
    })
    .await;

    // Stop the victim. Activate a rule it never sees.
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[3];
    test_cluster.stop_node(&victim);
    tokio::time::sleep(Duration::from_secs(10)).await;

    let denied = Address::new([44u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    for name in names.iter().skip(1).take(2) {
        restart_with_deny_config(&test_cluster, name, deny_config.clone()).await;
    }
    let live_walked = |name: &iota_types::base_types::AuthorityName| {
        let handle = test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .unwrap();
        walked_rules(&handle)
    };
    for attempt in 0..=120 {
        if names[..3]
            .iter()
            .all(|name| live_walked(name).is_some_and(|w| w.denied_addresses.contains(&denied)))
        {
            break;
        }
        assert!(attempt < 120, "the live validators never injected the rule");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Close the epoch on the live validators only. `force_new_epoch` waits
    // on every node and would hang on the stopped victim.
    let epoch = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch());
    for name in &names[..3] {
        test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .unwrap()
            .with_async(|node| async {
                node.close_epoch_for_testing().await.unwrap();
            })
            .await;
    }
    test_cluster.wait_for_epoch(Some(epoch + 1)).await;

    // The victim rejoins after the boundary. The missed commits are gone.
    // It catches up by executing synced checkpoints. A divergence report
    // would abort the simulation here.
    test_cluster.start_node(&victim).await;
    poll_all_validators(&test_cluster, "the catch-up convergence", |handle| {
        handle.with(|node| node.state().epoch_store_for_testing().epoch()) == epoch + 1
            && walked_rules(handle).is_some_and(|w| w.denied_addresses.contains(&denied))
    })
    .await;
    let victim_handle = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap();
    victim_handle.with(|node| {
        assert_eq!(
            node.state()
                .epoch_store_for_testing()
                .metrics_for_testing()
                .deny_rule_mirror_divergence
                .get(),
            0
        );
    });

    // The rejoined validator derives a later delta from its re-seeded mirror.
    let second = Address::new([45u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .add_denied_address(second)
        .build();
    for name in names.iter().skip(1).take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    poll_all_validators(&test_cluster, "the post-rejoin delta", |handle| {
        walked_rules(handle).is_some_and(|w| w.denied_addresses.contains(&second))
    })
    .await;
    test_cluster.force_new_epoch().await;
}

/// Crash after derivation, before the flush: the `crash` failpoint closure
/// watches the victim's injection counter, which the injecting commit bumps
/// before advancing the mirror and before the consensus handler's own
/// failpoint — so the victim dies inside the injecting commit, with the
/// mirror advanced and the commit durable in the consensus store but its
/// transactions never scheduled, executed, or flushed. The restart must
/// replay the commit and re-derive the same update, observed through the
/// fresh process's injection metrics and the canonical digests.
#[sim_test]
async fn deny_rule_crash_before_flush_replays_the_derivation() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iota_macros::register_fail_point;

    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let first = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(first)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the initial delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&first))
    })
    .await;

    // The injecting commit bumps the victim's injection counter right before
    // it advances the mirror, and its transactions are only scheduled after
    // the consensus handler's `crash` failpoint — so a failpoint closure that
    // watches the counter kills the victim inside the injecting commit
    // itself: mirror advanced, nothing scheduled, executed, or flushed.
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let victim_handle = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap();
    let victim_sim_id = victim_handle.with(|_| iota_simulator::current_simnode_id());
    let victim_metrics = victim_handle.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .metrics_for_testing()
            .clone()
    });
    let baseline = victim_metrics.deny_rule_update_transactions_injected.get();
    let fired = Arc::new(AtomicBool::new(false));
    {
        let fired = fired.clone();
        register_fail_point("crash", move || {
            if iota_simulator::current_simnode_id() == victim_sim_id
                && victim_metrics.deny_rule_update_transactions_injected.get() > baseline
                && !fired.swap(true, Ordering::SeqCst)
            {
                iota_simulator::task::kill_current_node(None);
            }
        });
    }

    // Activate a second rule; the victim dies deriving it.
    let second = Address::new([43u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(first)
        .add_denied_address(second)
        .build();
    for name in names.iter().skip(1).take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    for _ in 0..1200 {
        if fired.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(fired.load(Ordering::SeqCst), "the victim was never killed");

    // Restart and require the replay to re-derive: the fresh process's
    // injection counter must move, and the victim must converge on the
    // canonical digests. The held handle pins the killed node's store —
    // drop it and stop at the swarm level so the reopen can take the lock.
    drop(victim_handle);
    test_cluster.stop_node(&victim);
    tokio::time::sleep(Duration::from_secs(10)).await;
    test_cluster.start_node(&victim).await;
    let victim_handle = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap();
    for _ in 0..240 {
        if victim_handle.with(|node| {
            get_transaction_deny_rules(node.state().get_object_store().as_ref())
                .unwrap()
                .is_some_and(|walked| walked.denied_addresses.contains(&second))
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    victim_handle.with(|node| {
        let store = node.state().epoch_store_for_testing();
        let metrics = store.metrics_for_testing();
        assert!(
            metrics.deny_rule_update_transactions_injected.get() >= 1,
            "the replay must re-derive and re-schedule the unflushed update"
        );
        assert_eq!(
            metrics.deny_rule_update_execution_failures.get(),
            0,
            "no replayed update may fail execution"
        );
    });

    // The re-derivation produced the canonical digests; the victim's builder
    // re-checkpoints them shortly after the replay.
    let update_digests = checkpointed_update_digests(&test_cluster.fullnode_handle.iota_node);
    let all_checkpointed = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            update_digests
                .iter()
                .all(|digest| store.is_transaction_executed_in_checkpoint(digest).unwrap())
        })
    };
    for _ in 0..120 {
        if all_checkpointed(&victim_handle) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(
        all_checkpointed(&victim_handle),
        "the replayed derivation must match the canonical updates"
    );
    test_cluster.force_new_epoch().await;
}

/// A node that executed injected updates from outside the committee joins
/// it: its mirror seeds from the walked object, and it derives new deltas
/// identically to the incumbents.
#[sim_test]
async fn deny_rule_committee_joiner_derives_identical_deltas() {
    use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
    use iota_types::crypto::KeypairTraits;
    use rand::rngs::OsRng;

    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .build();
    let new_validator = ValidatorGenesisConfigBuilder::new().build(&mut OsRng);
    let joiner_name: iota_types::base_types::AuthorityName =
        new_validator.authority_key_pair.public().into();
    let candidate_address = new_validator.account_key_pair.public_key().derive_address();
    let mut test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_validator_candidates([candidate_address])
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    execute_join_transactions(&test_cluster, &new_validator).await;
    let joiner = test_cluster.spawn_new_validator(new_validator).await;

    // First boundary: the object is created and the incumbents inject the
    // delta; the joiner follows from outside the committee.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the injected delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&denied))
    })
    .await;
    joiner.with(|node| {
        let store = node.state().epoch_store_for_testing();
        assert!(
            !store.committee().authority_exists(&node.state().name),
            "the joiner must still be outside the committee"
        );
    });

    // Second boundary: the joiner enters the committee and seeds its mirror
    // from the walked object.
    test_cluster.force_new_epoch().await;
    joiner.with(|node| {
        let store = node.state().epoch_store_for_testing();
        assert!(
            store.committee().authority_exists(&node.state().name),
            "the joiner must be a committee member now"
        );
        assert!(
            store
                .get_mirrored_transaction_deny_rules()
                .denied_addresses
                .contains(&denied)
        );
    });

    // A rule activated now is derived by the joiner like every incumbent.
    let second = Address::new([43u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(denied)
        .add_denied_address(second)
        .build();
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().filter(|name| **name != joiner_name).take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    poll_all_validators(&test_cluster, "the post-join delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&second))
    })
    .await;

    // The boundary guard now covers the joiner's mirror as well.
    test_cluster.force_new_epoch().await;
    let reference = walked_rules(&joiner).unwrap();
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), reference);
    }
}

/// A kill switch activates immediately, and its deactivation is held through
/// the grace exactly like an entry removal: after the supporters withdraw,
/// the switch stays set on the object, in the mirror, and in enforcement.
#[sim_test]
async fn deny_rule_switch_holds_through_the_grace() {
    telemetry_subscribers::init_for_testing();
    // A floor no in-test commit round reaches: deactivations stay locked.
    let _guard = governance_overrides(1000, 100_000);

    let deny_config = TransactionDenyConfigBuilder::new()
        .disable_package_publish()
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    // Activation is immediate even while removals are locked.
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the switch activation", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.package_publish_disabled)
    })
    .await;

    // Withdraw the switch on three of four validators: support falls below
    // 2f+1, but the deactivation must wait for the grace.
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }
    tokio::time::sleep(Duration::from_secs(10)).await;
    for handle in test_cluster.swarm.validator_node_handles() {
        assert!(
            walked_rules(&handle).unwrap().package_publish_disabled,
            "a deactivation must not reach the object during the grace"
        );
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            assert!(
                store
                    .get_mirrored_transaction_deny_rules()
                    .package_publish_disabled
            );
            assert!(
                store
                    .get_active_transaction_deny_rules()
                    .package_publish_disabled,
                "enforcement must not lapse while the deactivation is held"
            );
        });
    }
}

/// With the grace floor at zero, a withdrawn kill switch clears from the
/// object deterministically at the unlock.
#[sim_test]
async fn deny_rule_switch_deactivation_clears_at_unlock() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let deny_config = TransactionDenyConfigBuilder::new()
        .disable_package_publish()
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the switch activation", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.package_publish_disabled)
    })
    .await;

    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(3) {
        restart_with_deny_config(
            &test_cluster,
            name,
            TransactionDenyConfigBuilder::new().build(),
        )
        .await;
    }
    poll_all_validators(&test_cluster, "the switch deactivation", |handle| {
        walked_rules(handle).is_some_and(|walked| !walked.package_publish_disabled)
    })
    .await;
}

/// A delta at the chunk limit the flip is expected to use executes through
/// the real pipeline: 1000 entries in a single update transaction reach the
/// object on every node.
#[sim_test]
async fn deny_rule_shipped_chunk_limit_executes_in_the_cluster() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let denied: Vec<Address> = (0..1000u32)
        .map(|i| {
            let mut bytes = [0u8; 32];
            bytes[..4].copy_from_slice(&i.to_be_bytes());
            Address::new(bytes)
        })
        .collect();
    let mut builder = TransactionDenyConfigBuilder::new();
    for address in &denied {
        builder = builder.add_denied_address(*address);
    }
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(builder.build())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    let expected: std::collections::BTreeSet<Address> = denied.iter().copied().collect();
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the full-chunk delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses == expected)
    })
    .await;
    poll_fullnode(&test_cluster, "the full-chunk delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses == expected)
    })
    .await;
}

/// A chunked injection keeps the metric relationship — one injecting commit,
/// one transaction per chunk — on every validator, and the injected update
/// renders through the existing JSON-RPC system-transaction path.
#[sim_test]
async fn deny_rule_injection_metrics_and_rpc_rendering() {
    use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
    use jsonrpsee::{core::client::ClientT, rpc_params};

    telemetry_subscribers::init_for_testing();
    // 7 entries at 3 per transaction: one commit, three chunks.
    let _guard = governance_overrides(3, 0);

    let denied: Vec<Address> = (0..7u8).map(|i| Address::new([i + 1; 32])).collect();
    let mut builder = TransactionDenyConfigBuilder::new();
    for address in &denied {
        builder = builder.add_denied_address(*address);
    }
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(builder.build())
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the chunked delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.len() == denied.len())
    })
    .await;

    for handle in test_cluster.swarm.validator_node_handles() {
        handle.with(|node| {
            let store = node.state().epoch_store_for_testing();
            let metrics = store.metrics_for_testing();
            assert_eq!(
                metrics.deny_rule_updates_injected.get(),
                1,
                "the whole delta injects in one commit"
            );
            assert_eq!(
                metrics.deny_rule_update_transactions_injected.get(),
                3,
                "one transaction per chunk"
            );
            assert_eq!(
                metrics.deny_rule_update_execution_failures.get(),
                0,
                "no injected update may fail execution"
            );
        });
    }

    // The injected update renders through JSON-RPC.
    let digest = 'digest: {
        for _ in 0..120 {
            let digests = checkpointed_update_digests(&test_cluster.fullnode_handle.iota_node);
            if let Some(digest) = digests.first() {
                break 'digest *digest;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("no update reached the fullnode's checkpoints");
    };
    let response: iota_json_rpc_types::IotaTransactionBlockResponse = test_cluster
        .fullnode_handle
        .rpc_client
        .request(
            "iota_getTransactionBlock",
            rpc_params![
                digest,
                IotaTransactionBlockResponseOptions::new().with_input()
            ],
        )
        .await
        .expect("the injected update must be servable over JSON-RPC");
    let rendered = serde_json::to_string(&response).unwrap();
    assert!(
        rendered.contains("TransactionDenyRulesUpdate"),
        "the update must render as its own kind: {rendered}"
    );
}

/// Repeated restarts under continuous rule churn converge: every round flips
/// the rule set while a validator restarts mid-flight, and each boundary
/// re-checks every mirror against the object.
#[sim_test]
async fn deny_rule_churn_with_restarts_converges() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let first = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(first)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the initial delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&first))
    })
    .await;

    let names = test_cluster.get_validator_pubkeys();
    for round in 0u8..3 {
        // Each round denies a fresh address on two validators (activation at
        // f+1) while a third validator restarts concurrently — its replay
        // races the round's injection.
        let fresh = Address::new([100 + round; 32]);
        let config = TransactionDenyConfigBuilder::new()
            .add_denied_address(first)
            .add_denied_address(fresh)
            .build();
        // Always a non-supporter, so the restart genuinely races the
        // round's injection instead of being one of its activators.
        let restarting = names[2 + (round as usize % 2)];
        test_cluster.stop_node(&restarting);
        for name in names.iter().take(2) {
            restart_with_deny_config(&test_cluster, name, config.clone()).await;
        }
        test_cluster.start_node(&restarting).await;

        poll_all_validators(&test_cluster, "the round's delta", |handle| {
            walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&fresh))
        })
        .await;
        // A boundary per round: any drifted mirror aborts the simulation.
        test_cluster.force_new_epoch().await;
    }

    let reference = walked_rules(&test_cluster.swarm.validator_node_handles()[0]).unwrap();
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), reference);
        handle.with(|node| {
            assert_eq!(
                *node
                    .state()
                    .epoch_store_for_testing()
                    .get_mirrored_transaction_deny_rules(),
                reference
            );
        });
    }
}

/// A delta racing the epoch's close is either injected and checkpointed or
/// held back with the mirror — never advanced and lost. The boundary guard
/// and the next epoch's convergence are the arbiters for whichever side of
/// the race the seed produces.
#[sim_test]
async fn deny_rule_delta_racing_the_epoch_close_is_never_lost() {
    telemetry_subscribers::init_for_testing();
    let _guard = governance_overrides(1000, 0);

    let first = Address::new([42u8; 32]);
    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(first)
        .build();
    let test_cluster = TestClusterBuilder::new()
        .with_transaction_deny_config(deny_config)
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    poll_all_validators(&test_cluster, "the initial delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&first))
    })
    .await;

    // Activate a rule and close the epoch immediately: the injection lands
    // on one of the final commits or is skipped after `close_all_tx`.
    let second = Address::new([43u8; 32]);
    let both = TransactionDenyConfigBuilder::new()
        .add_denied_address(first)
        .add_denied_address(second)
        .build();
    let names = test_cluster.get_validator_pubkeys();
    for name in names.iter().take(2) {
        restart_with_deny_config(&test_cluster, name, both.clone()).await;
    }
    test_cluster.force_new_epoch().await;

    // Whichever way the race went, the next epoch re-derives anything held
    // back, and the object converges without a fork.
    poll_all_validators(&test_cluster, "the racing delta", |handle| {
        walked_rules(handle).is_some_and(|walked| walked.denied_addresses.contains(&second))
    })
    .await;
    test_cluster.force_new_epoch().await;
    let reference = walked_rules(&test_cluster.swarm.validator_node_handles()[0]).unwrap();
    for handle in test_cluster.swarm.validator_node_handles() {
        assert_eq!(walked_rules(&handle).unwrap(), reference);
    }
}
