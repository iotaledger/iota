// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use iota_data_ingestion_core::Worker;
use iota_kvstore::{
    KeyValueStoreReader, KeyValueStoreWriter, KvWorker,
    client::{TransactionSequenceNumber, TransactionsOrder},
    emulator::BigTableEmulator,
    transactions_by_address,
};
use iota_sdk_types::Address;
use iota_types::{
    digests::TransactionDigest, full_checkpoint_content::CheckpointData,
    test_checkpoint_data_builder::TestCheckpointDataBuilder,
};

/// Build a small checkpoint covering affected-address cases.
fn build_test_checkpoint() -> CheckpointData {
    TestCheckpointDataBuilder::new(1)
        // tx 0: sender 0 creates object 0 (owned by sender 0)
        //   → affected = {addr_0}
        .start_transaction(0)
        .create_owned_object(0)
        .finish_transaction()
        // tx 1: sender 0 transfers object 0 to addr 1
        //   → affected = {addr_0, addr_1}
        .start_transaction(0)
        .transfer_object(0, 1)
        .finish_transaction()
        // tx 2: sender 2 creates object 1 (owned by sender 2)
        //   → affected = {addr_2}
        .start_transaction(2)
        .create_owned_object(1)
        .finish_transaction()
        // tx 3: sender 2 transfers object 1 to addr 3
        //   → affected = {addr_2, addr_3}
        .start_transaction(2)
        .transfer_object(1, 3)
        .finish_transaction()
        .build_checkpoint()
}

#[test]
fn affected_addresses_per_transaction() {
    let checkpoint = build_test_checkpoint();
    let addr = TestCheckpointDataBuilder::derive_address;

    let first_seq = checkpoint
        .checkpoint_contents
        .enumerate_transactions(&checkpoint.checkpoint_summary)
        .next()
        .unwrap()
        .0;
    let tx_seq_to_digest = |i: usize| *checkpoint.transactions[i].transaction.digest();

    let expected = HashSet::from([
        // tx 0: only sender 0 (sender == payer == sole recipient)
        (addr(0), first_seq, tx_seq_to_digest(0)),
        // tx 1: sender 0 (also payer) + recipient 1
        (addr(0), first_seq + 1, tx_seq_to_digest(1)),
        (addr(1), first_seq + 1, tx_seq_to_digest(1)),
        // tx 2: only sender 2
        (addr(2), first_seq + 2, tx_seq_to_digest(2)),
        // tx 3: sender 2 (also payer) + recipient 3
        (addr(2), first_seq + 3, tx_seq_to_digest(3)),
        (addr(3), first_seq + 3, tx_seq_to_digest(3)),
    ]);

    let actual: HashSet<(Address, TransactionSequenceNumber, TransactionDigest)> =
        transactions_by_address(&checkpoint).collect();
    assert_eq!(actual, expected);
}

#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn process_checkpoint() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();
    let checkpoint = build_test_checkpoint();

    // capture expected (address -> set of digests) BEFORE the checkpoint is moved.
    let expected_rows = transactions_by_address(&checkpoint);
    let mut expected_by_address: HashMap<
        Address,
        HashMap<TransactionSequenceNumber, TransactionDigest>,
    > = HashMap::new();
    for (address, seq, digest) in expected_rows {
        expected_by_address
            .entry(address)
            .or_default()
            .insert(seq, digest);
    }

    let kv_worker = KvWorker::new(client.clone());
    // write to BigTable: run the worker on the checkpoint.
    kv_worker
        .process_checkpoint(checkpoint.into())
        .await
        .unwrap();

    // read back from BigTable and compare per affected address.
    for (address, expected_digests) in &expected_by_address {
        let fetched = client
            .get_transaction_digests_by_address(*address, None, 10, Default::default())
            .await
            .unwrap()
            .into_iter()
            .collect::<HashMap<TransactionSequenceNumber, TransactionDigest>>();

        assert_eq!(
            &fetched, expected_digests,
            "digests stored for {address:?} do not match expected",
        );
    }

    // addresses we never used should return nothing.
    let unused = TestCheckpointDataBuilder::derive_address(99);
    let fetched = client
        .get_transaction_digests_by_address(unused, None, 10, Default::default())
        .await
        .unwrap();

    assert!(
        fetched.is_empty(),
        "unrelated address {unused:?} unexpectedly has stored digests",
    );
}

#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn paginates_newest_first() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    let address = Address::random();
    let seqs = [10u64, 20, 30, 40, 50];
    let digests = std::iter::repeat_n(TransactionDigest::random(), seqs.len())
        .collect::<Vec<TransactionDigest>>();
    let entries = seqs
        .iter()
        .zip(digests.iter())
        .map(|(s, d)| (address, *s, *d))
        .collect::<Vec<_>>();
    client.save_transactions_by_address(entries).await.unwrap();

    // page 1: 3 newest, in newest-first order.
    let page1 = client
        .get_transaction_digests_by_address(address, None, 3, TransactionsOrder::NewestFirst)
        .await
        .unwrap();
    assert_eq!(
        page1,
        vec![(50, digests[4]), (40, digests[3]), (30, digests[2])]
    );

    // page 2: continue past seq 30, no overlap with page 1.
    let page2 = client
        .get_transaction_digests_by_address(address, Some(30), 10, TransactionsOrder::NewestFirst)
        .await
        .unwrap();
    assert_eq!(page2, vec![(20, digests[1]), (10, digests[0])]);
}

#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn paginates_oldest_first() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    let address = Address::random();
    let seqs = [10u64, 20, 30, 40, 50];
    let digests = std::iter::repeat_n(TransactionDigest::random(), seqs.len())
        .collect::<Vec<TransactionDigest>>();
    let entries = seqs
        .iter()
        .zip(digests.iter())
        .map(|(s, d)| (address, *s, *d))
        .collect::<Vec<_>>();
    client.save_transactions_by_address(entries).await.unwrap();

    // page 1: 3 oldest, in oldest-first order.
    let page1 = client
        .get_transaction_digests_by_address(address, None, 3, TransactionsOrder::OldestFirst)
        .await
        .unwrap();
    assert_eq!(
        page1,
        vec![(10, digests[0]), (20, digests[1]), (30, digests[2])]
    );

    // page 2: continue past seq 30, no overlap with page 1.
    let page2 = client
        .get_transaction_digests_by_address(address, Some(30), 10, TransactionsOrder::OldestFirst)
        .await
        .unwrap();
    assert_eq!(page2, vec![(40, digests[3]), (50, digests[4])]);
}

#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn order_flip_is_exact_reverse() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    let address = Address::random();
    let seqs = [10u64, 20, 30, 50];
    let digests = std::iter::repeat_n(TransactionDigest::random(), seqs.len())
        .collect::<Vec<TransactionDigest>>();
    let entries = seqs
        .iter()
        .zip(digests.iter())
        .map(|(s, d)| (address, *s, *d))
        .collect::<Vec<_>>();
    client.save_transactions_by_address(entries).await.unwrap();

    let newest_first = client
        .get_transaction_digests_by_address(address, None, 10, TransactionsOrder::NewestFirst)
        .await
        .unwrap();
    let oldest_first = client
        .get_transaction_digests_by_address(address, None, 10, TransactionsOrder::OldestFirst)
        .await
        .unwrap();

    assert_eq!(
        newest_first,
        vec![
            (50, digests[3]),
            (30, digests[2]),
            (20, digests[1]),
            (10, digests[0])
        ]
    );
    assert_eq!(
        oldest_first,
        vec![
            (10, digests[0]),
            (20, digests[1]),
            (30, digests[2]),
            (50, digests[3])
        ]
    );
    let mut reversed = oldest_first;
    reversed.reverse();
    assert_eq!(newest_first, reversed);
}

#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn empty_range_guards_return_ok_empty() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    // Address need not exist: the guards short-circuit before hitting BigTable.
    let address = Address::random();

    // NewestFirst with cursor = 0: nothing is older than seq 0.
    // BigTable would reject the empty range; the guard must return Ok(vec![]).
    let res = client
        .get_transaction_digests_by_address(address, Some(0), 10, TransactionsOrder::NewestFirst)
        .await
        .unwrap();
    assert!(res.is_empty());

    // OldestFirst with cursor = u64::MAX: nothing is newer than u64::MAX.
    let res = client
        .get_transaction_digests_by_address(
            address,
            Some(u64::MAX),
            10,
            TransactionsOrder::OldestFirst,
        )
        .await
        .unwrap();
    assert!(res.is_empty());
}
