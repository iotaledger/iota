// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Cluster tests for the P-COOL deterministic-validation bookkeeping.
//!
//! Gated to the simulator for the reason given in
//! `pcool_transaction_flow_tests.rs`: the protocol-config override that turns
//! the feature on is thread-local, and only the simulator runs every node on
//! the test's thread.
//!
//! Run with: `cargo simtest -p iota-e2e-tests pcool_deterministic_validation`.

#![cfg(msim)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use iota_macros::{register_fail_point, sim_test};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::TransactionEffects;
use iota_types::{base_types::dbg_addr, effects::TransactionEffectsExt, storage::ObjectKey};
use test_cluster::{TestCluster, TestClusterBuilder};

/// Enables the P-COOL flow and the deterministic-validation bookkeeping. The
/// guard must be held for the whole test.
fn enable_deterministic_validation_for_testing() -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_pcool_deterministic_validation_for_testing(true);
        config
    })
}

async fn transfer(test_cluster: &TestCluster) -> TransactionEffects {
    let tx = test_cluster
        .test_transaction_builder_with_sender(test_cluster.get_address_0())
        .await
        .transfer_iota(Some(1_000_000), dbg_addr(2))
        .build();
    test_cluster.sign_and_execute_transaction(&tx).await
}

/// The keys of the handler rows a transaction's execution produces: every
/// object it wrote, at the version it wrote.
fn written_keys(effects: &TransactionEffects) -> Vec<ObjectKey> {
    effects
        .all_changed_objects()
        .into_iter()
        .map(|(owned_ref, _)| {
            ObjectKey(
                owned_ref.reference().object_id,
                owned_ref.reference().version,
            )
        })
        .collect()
}

/// A validator killed after a checkpoint's bookkeeping batch is durable but
/// before the checkpoint's outputs are re-executes the checkpoint on restart,
/// keeps executing checkpoints, and ends up with the same handler rows as its
/// peers.
#[sim_test]
async fn test_crash_after_checkpoint_bookkeeping_write_recovers() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    let names = test_cluster.get_validator_pubkeys();
    let (victim, peer) = (names[0], names[1]);
    let node_handle = |name| {
        test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap()
    };
    // Read without keeping the handle: a held handle pins the killed node's
    // store, and the restarted node could not open it.
    let victim_sim_id = node_handle(victim).with(|node| {
        assert!(
            node.state()
                .epoch_store_for_testing()
                .protocol_config()
                .pcool_deterministic_validation(),
            "the protocol-config override must reach the validators"
        );
        iota_simulator::current_simnode_id()
    });

    let fired = Arc::new(AtomicBool::new(false));
    {
        let fired = fired.clone();
        register_fail_point("crash-after-checkpoint-bookkeeping-write", move || {
            if iota_simulator::current_simnode_id() == victim_sim_id
                && !fired.swap(true, Ordering::SeqCst)
            {
                iota_simulator::task::kill_current_node(None);
            }
        });
    }

    // The victim dies in the checkpoint that carries this transaction's rows,
    // and restarts on its own.
    let mut transfers = vec![transfer(&test_cluster).await];
    tokio::time::timeout(Duration::from_secs(60), async {
        while !fired.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the victim must write a checkpoint's bookkeeping and crash");
    for _ in 0..3 {
        transfers.push(transfer(&test_cluster).await);
    }

    // The victim executes past every checkpoint the fullnode has.
    let target = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
        })
        .unwrap()
        .expect("the fullnode has executed the transfers' checkpoints");
    let highest_executed = |name| {
        node_handle(name).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    tokio::time::timeout(Duration::from_secs(120), async {
        while highest_executed(victim) < target {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the restarted victim must keep executing checkpoints");

    // Its handler rows match the peer's for every transfer, including the one
    // whose checkpoint it re-executed. The rows may still be on their way
    // while the victim's handler replays, so wait for them.
    let rows = |name, key: ObjectKey| {
        node_handle(name).with(|node| {
            node.state()
                .epoch_store_for_testing()
                .handler_processed_object(&key)
                .unwrap()
        })
    };
    for key in transfers.iter().flat_map(written_keys) {
        let expected = rows(peer, key).expect("the peer has a handler row for every transfer");
        tokio::time::timeout(Duration::from_secs(60), async {
            while rows(victim, key) != Some(expected) {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the victim's handler row at {key:?} must match the peer's: {:?} vs {expected:?}",
                rows(victim, key)
            )
        });
    }
}
