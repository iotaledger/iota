// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The CLI stays fully functional when the node serves only gRPC, by pointing
//! the wallet env's `rpc` at an indexer reader (Strategy Y).
//!
//! Needs a real Postgres (the indexer store), so it runs on real tokio and is
//! excluded from the deterministic simulator (`cargo simtest`).
#![cfg(not(msim))]

mod common;

use common::{cluster_with_indexer_backed_wallet, wait_for_indexer, wait_for_transaction_on_node};
use iota::{
    client_commands::{
        GasDataArgs, IotaClientCommandResult, IotaClientCommands, PaymentArgs, TxProcessingArgs,
    },
    key_identity::KeyIdentity,
};
use iota_sdk_types::Address;
use iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER;
use test_cluster::TestClusterBuilder;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_commands_work_over_indexer_backed_wallet() {
    let (mut cluster, pg_store) =
        cluster_with_indexer_backed_wallet("cli_indexer_tests", TestClusterBuilder::new()).await;
    let address = cluster.get_address_0();

    // The CLI reads through the indexer, so it must be caught up first.
    wait_for_indexer(&pg_store, &cluster).await;

    // Reads resolve through the indexer.
    let objects = IotaClientCommands::Objects {
        address: Some(KeyIdentity::Address(address)),
    }
    .execute(&mut cluster.wallet)
    .await
    .expect("`iota client objects` should read through the indexer");
    let IotaClientCommandResult::Objects(objects) = objects else {
        panic!("expected an Objects result");
    };
    assert!(!objects.is_empty(), "address should own objects");

    let balance = IotaClientCommands::Balance {
        address: None,
        coin_type: None,
        with_coins: false,
    }
    .execute(&mut cluster.wallet)
    .await
    .expect("`iota client balance` should read through the indexer");
    let IotaClientCommandResult::Balance(coins, _) = balance else {
        panic!("expected a Balance result");
    };
    assert!(
        coins.iter().any(|(_, coins)| !coins.is_empty()),
        "address should hold a positive balance"
    );

    // Execution reads gas price / protocol config / dry-run over the indexer,
    // then submits over the node gRPC.
    let gas_objects = cluster
        .wallet
        .get_gas_objects_owned_by_address(address, None)
        .await
        .unwrap();
    let transferred = gas_objects[0].object_id;
    let rgp = cluster.get_reference_gas_price().await;
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(Address::random()),
        object_id: transferred,
        payment: PaymentArgs {
            gas: vec![gas_objects[1].object_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(&mut cluster.wallet)
    .await
    .expect("`iota client transfer` should execute over gRPC + indexer");
    let IotaClientCommandResult::TransactionBlock(transfer) = transfer else {
        panic!("expected a TransactionBlock result");
    };
    assert!(
        transfer.status_ok().unwrap(),
        "transfer failed: {transfer:?}"
    );

    // Execution ran over the node gRPC; confirm the node executed it, then wait
    // for the indexer to catch up before reading the new state back.
    wait_for_transaction_on_node(&cluster, transfer.digest).await;
    wait_for_indexer(&pg_store, &cluster).await;
    let objects = IotaClientCommands::Objects {
        address: Some(KeyIdentity::Address(address)),
    }
    .execute(&mut cluster.wallet)
    .await
    .expect("post-transfer `iota client objects` should read through the indexer");
    let IotaClientCommandResult::Objects(objects) = objects else {
        panic!("expected an Objects result");
    };
    assert!(
        !objects
            .iter()
            .any(|o| o.data.as_ref().map(|d| d.object_id) == Some(transferred)),
        "the transferred object should no longer be owned by the sender"
    );
}
