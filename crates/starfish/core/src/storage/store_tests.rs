// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use rstest::rstest;
use starfish_config::AuthorityIndex;
use tempfile::TempDir;

use super::{Store, WriteBatch, mem_store::MemStore, rocksdb_store::RocksDBStore};
use crate::{
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, Slot, TestBlockHeader, VerifiedBlock,
    },
    commit::{CommitDigest, CommitRef, TrustedCommit},
    context::Context,
    transaction_ref::GenericTransactionRef,
};

/// Test fixture for store tests. Wraps around various store implementations.
#[expect(clippy::large_enum_variant)]
enum TestStore {
    RocksDB((RocksDBStore, TempDir, bool)),
    Mem(MemStore, bool),
}

impl TestStore {
    fn store(&self) -> &dyn Store {
        match self {
            TestStore::RocksDB((store, _, _)) => store,
            TestStore::Mem(store, _) => store,
        }
    }

    fn consensus_fast_commit_sync(&self) -> bool {
        match self {
            TestStore::RocksDB((_, _, enabled)) => *enabled,
            TestStore::Mem(_, enabled) => *enabled,
        }
    }

    /// Creates a context with the same configuration as the store.
    fn context(&self) -> Arc<Context> {
        let (mut context, _) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(self.consensus_fast_commit_sync());
        context.parameters.enable_fast_commit_syncer = self.consensus_fast_commit_sync();
        Arc::new(context)
    }
}

fn new_rocksdb_teststore(consensus_fast_commit_sync: bool) -> TestStore {
    let (mut context, _) = Context::new_for_test(4);
    context
        .protocol_config
        .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
    context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
    let temp_dir = TempDir::new().unwrap();
    TestStore::RocksDB((
        RocksDBStore::new(temp_dir.path().to_str().unwrap(), Arc::new(context)),
        temp_dir,
        consensus_fast_commit_sync,
    ))
}

fn new_mem_teststore(consensus_fast_commit_sync: bool) -> TestStore {
    let (mut context, _) = Context::new_for_test(4);
    context
        .protocol_config
        .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
    context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
    TestStore::Mem(
        MemStore::new(Arc::from(context.clone())),
        consensus_fast_commit_sync,
    )
}

#[rstest]
#[tokio::test]
async fn read_and_contain_block_headers(
    #[values(new_rocksdb_teststore(false), new_mem_teststore(false))] test_store: TestStore,
) {
    let store = test_store.store();

    let (context, _) = Context::new_for_test(4);

    let written_blocks: Vec<VerifiedBlock> = vec![
        VerifiedBlock::new_for_test(TestBlockHeader::new(1, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(1, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(1, 2).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(2, 3).build()),
    ];

    // Write only headers
    store
        .write(
            WriteBatch::default().block_headers(
                written_blocks
                    .iter()
                    .map(|b| b.verified_block_header.clone())
                    .collect(),
            ),
            Arc::from(context),
        )
        .unwrap();

    // Test basic header read
    let refs = vec![written_blocks[0].reference()];
    let read_headers = store
        .read_verified_block_headers(&refs)
        .expect("Read headers should not fail");
    assert_eq!(read_headers.len(), 1);
    assert_eq!(
        read_headers[0].as_ref().unwrap(),
        &written_blocks[0].verified_block_header
    );

    // Test multiple references including duplicates
    let refs = vec![
        written_blocks[2].reference(),
        written_blocks[1].reference(),
        written_blocks[1].reference(),
    ];
    let read_headers = store
        .read_verified_block_headers(&refs)
        .expect("Read headers should not fail");
    assert_eq!(read_headers.len(), 3);
    assert_eq!(
        read_headers[0].as_ref().unwrap(),
        &written_blocks[2].verified_block_header
    );
    assert_eq!(
        read_headers[1].as_ref().unwrap(),
        &written_blocks[1].verified_block_header
    );
    assert_eq!(
        read_headers[2].as_ref().unwrap(),
        &written_blocks[1].verified_block_header
    );

    // Test with missing references
    let refs = vec![
        written_blocks[3].reference(),
        BlockRef::new(
            1,
            AuthorityIndex::new_for_test(3),
            BlockHeaderDigest::default(),
        ),
        written_blocks[2].reference(),
    ];
    let read_headers = store
        .read_verified_block_headers(&refs)
        .expect("Read headers should not fail");
    assert_eq!(read_headers.len(), 3);
    assert_eq!(
        read_headers[0].as_ref().unwrap(),
        &written_blocks[3].verified_block_header
    );
    assert!(read_headers[1].is_none());
    assert_eq!(
        read_headers[2].as_ref().unwrap(),
        &written_blocks[2].verified_block_header
    );

    let contains = store
        .contains_block_headers(&refs)
        .expect("Contains headers should not fail");
    assert_eq!(contains.len(), 3);
    assert!(contains[0]);
    assert!(!contains[1]);
    assert!(contains[2]);

    // Test slot existence
    for block in &written_blocks {
        let found = store
            .contains_block_at_slot(block.slot())
            .expect("Check slot should not fail");
        assert!(found);
    }

    let found = store
        .contains_block_at_slot(Slot::new(10, AuthorityIndex::new_for_test(0)))
        .expect("Check slot should not fail");
    assert!(!found);
}

#[rstest]
#[tokio::test]
async fn scan_block_headers(
    #[values(
        new_rocksdb_teststore(false),
        new_mem_teststore(false),
        new_rocksdb_teststore(true),
        new_mem_teststore(true)
    )]
    test_store: TestStore,
) {
    let store = test_store.store();
    let (mut context, _) = crate::context::Context::new_for_test(4);
    // Match the flag used to create the store
    let consensus_fast_commit_sync = test_store.consensus_fast_commit_sync();
    context
        .protocol_config
        .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
    context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
    let context = Arc::new(context);

    let written_blocks = [
        VerifiedBlock::new_for_test(TestBlockHeader::new(9, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(10, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(10, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(11, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(11, 3).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(12, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(13, 2).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(13, 1).build()),
    ];

    // Write block headers
    store
        .write(
            WriteBatch::default()
                .block_headers(
                    written_blocks
                        .iter()
                        .map(|b| b.verified_block_header.clone())
                        .collect(),
                )
                .transactions(
                    written_blocks
                        .iter()
                        .map(|b| b.verified_transactions.clone())
                        .collect(),
                ),
            context.clone(),
        )
        .unwrap();

    // Test scanning with no results
    let scanned_headers = store
        .scan_block_headers_by_author(AuthorityIndex::new_for_test(4), 20)
        .expect("Scan headers should not fail");
    assert!(scanned_headers.is_empty(), "{scanned_headers:?}");

    // Test scanning with specific start round
    let scanned_headers = store
        .scan_block_headers_by_author(AuthorityIndex::new_for_test(1), 12)
        .expect("Scan headers should not fail");
    assert_eq!(scanned_headers.len(), 2, "{scanned_headers:?}");
    assert_eq!(
        scanned_headers,
        vec![
            written_blocks[5].verified_block_header.clone(),
            written_blocks[7].verified_block_header.clone()
        ]
    );

    // Add more headers and test scanning
    let additional_blocks = [
        VerifiedBlock::new_for_test(TestBlockHeader::new(14, 2).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(15, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(15, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(16, 3).build()),
    ];

    // Write additional block headers
    store
        .write(
            WriteBatch::default()
                .block_headers(
                    additional_blocks
                        .iter()
                        .map(|b| b.verified_block_header.clone())
                        .collect(),
                )
                .transactions(
                    additional_blocks
                        .iter()
                        .map(|b| b.verified_transactions.clone())
                        .collect(),
                ),
            context,
        )
        .unwrap();
    {
        let scanned_headers = store
            .scan_block_headers_by_author(AuthorityIndex::new_for_test(1), 10)
            .expect("Scan headers should not fail");
        assert_eq!(scanned_headers.len(), 5);
        assert_eq!(
            scanned_headers,
            vec![
                written_blocks[2].verified_block_header.clone(),
                written_blocks[3].verified_block_header.clone(),
                written_blocks[5].verified_block_header.clone(),
                written_blocks[7].verified_block_header.clone(),
                additional_blocks[2].verified_block_header.clone(),
            ]
        );
    }

    {
        let scanned_blocks = store
            .scan_last_blocks_by_author(AuthorityIndex::new_for_test(1), 2, None)
            .expect("Scan blocks should not fail");
        assert_eq!(scanned_blocks.len(), 2, "{scanned_blocks:?}");
        assert_eq!(
            scanned_blocks,
            vec![written_blocks[7].clone(), additional_blocks[2].clone()]
        );

        let scanned_blocks = store
            .scan_last_blocks_by_author(AuthorityIndex::new_for_test(1), 0, None)
            .expect("Scan blocks should not fail");
        assert_eq!(scanned_blocks.len(), 0);
    }
}

#[rstest]
#[tokio::test]
async fn read_and_contain_transactions(
    #[values(
        new_rocksdb_teststore(false),
        new_mem_teststore(false),
        new_rocksdb_teststore(true),
        new_mem_teststore(true)
    )]
    test_store: TestStore,
) {
    let store = test_store.store();

    let written_blocks = [
        VerifiedBlock::new_for_test(TestBlockHeader::new(9, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(10, 0).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(10, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(11, 1).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(11, 3).build()),
        VerifiedBlock::new_for_test(TestBlockHeader::new(12, 1).build()),
    ];
    let (mut context, _) = Context::new_for_test(4);
    // Match the flag used to create the store
    let consensus_fast_commit_sync = test_store.consensus_fast_commit_sync();
    context
        .protocol_config
        .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
    context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
    let context = Arc::new(context);
    // Write transactions to store
    let written_transactions: Vec<_> = written_blocks
        .iter()
        .map(|b| b.verified_transactions.clone())
        .collect();
    store
        .write(
            WriteBatch::default().transactions(written_transactions),
            context.clone(),
        )
        .unwrap();
    // Also write headers since we read transaction commitment from headers now
    let written_headers = written_blocks
        .iter()
        .map(|b| b.verified_block_header.clone())
        .collect();
    store
        .write(
            WriteBatch::default().block_headers(written_headers),
            context,
        )
        .unwrap();

    // Test reading all transactions
    let refs: Vec<_> = if consensus_fast_commit_sync {
        // When flag is enabled, use TransactionRef
        written_blocks
            .iter()
            .map(|b| {
                GenericTransactionRef::TransactionRef(b.verified_block_header.transaction_ref())
            })
            .collect()
    } else {
        // When flag is disabled, use BlockRef
        written_blocks
            .iter()
            .map(|b| GenericTransactionRef::from(b.reference()))
            .collect()
    };
    let read_txs = store
        .read_verified_transactions(&refs)
        .expect("Read txs should not fail");

    assert_eq!(read_txs.len(), written_blocks.len());
    for (i, tx_opt) in read_txs.iter().enumerate() {
        let expected = &written_blocks[i].verified_transactions;
        let actual = tx_opt.as_ref().unwrap();
        assert_eq!(actual, expected);

        if !consensus_fast_commit_sync {
            assert_eq!(
                tx_opt.as_ref().unwrap().block_ref().unwrap(),
                written_blocks[i].reference()
            );
        } else {
            assert_eq!(
                tx_opt.as_ref().unwrap().transaction_ref(),
                written_blocks[i].verified_block_header.transaction_ref()
            );
        }
    }

    // Test reading subset of transactions
    let subset_refs = vec![refs[1], refs[3], refs[5]];
    let read_subset = store
        .read_verified_transactions(&subset_refs)
        .expect("Read subset should not fail");
    assert_eq!(read_subset.len(), 3);
    assert_eq!(
        read_subset[0].as_ref().unwrap(),
        &written_blocks[1].verified_transactions
    );
    assert_eq!(
        read_subset[1].as_ref().unwrap(),
        &written_blocks[3].verified_transactions
    );
    assert_eq!(
        read_subset[2].as_ref().unwrap(),
        &written_blocks[5].verified_transactions
    );

    // Test existence checks
    let contains = store
        .contains_transactions(&refs)
        .expect("Contains txs should not fail");
    assert_eq!(contains, vec![true; refs.len()]);

    // Test with missing reference
    let missing_ref = GenericTransactionRef::from(BlockRef::new(
        99,
        AuthorityIndex::new_for_test(99),
        BlockHeaderDigest::default(),
    ));
    let read_missing = store
        .read_verified_transactions(&[missing_ref])
        .expect("Read missing should not fail");
    assert_eq!(read_missing.len(), 1);
    assert!(read_missing[0].is_none());

    let contains_missing = store
        .contains_transactions(&[missing_ref])
        .expect("Contains missing should not fail");
    assert_eq!(contains_missing, vec![false]);
}

#[rstest]
#[tokio::test]
async fn read_and_scan_commits(
    #[values(new_rocksdb_teststore(false), new_mem_teststore(false))] test_store: TestStore,
) {
    let store = test_store.store();
    let (context, _) = Context::new_for_test(4);
    let context = Arc::new(context);

    {
        let last_commit = store
            .read_last_commit()
            .expect("Read last commit should not fail");
        assert!(last_commit.is_none(), "{last_commit:?}");
    }

    let written_commits = vec![
        TrustedCommit::new_for_test(
            &context,
            1,
            CommitDigest::MIN,
            1,
            BlockRef::new(
                1,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            vec![],
            vec![],
        ),
        TrustedCommit::new_for_test(
            &context,
            2,
            CommitDigest::MIN,
            2,
            BlockRef::new(
                2,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            vec![],
            vec![],
        ),
        TrustedCommit::new_for_test(
            &context,
            3,
            CommitDigest::MIN,
            3,
            BlockRef::new(
                3,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            vec![],
            vec![],
        ),
        TrustedCommit::new_for_test(
            &context,
            4,
            CommitDigest::MIN,
            4,
            BlockRef::new(
                4,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            vec![],
            vec![],
        ),
    ];
    store
        .write(
            WriteBatch::default().commits(written_commits.clone()),
            context,
        )
        .unwrap();

    {
        let last_commit = store
            .read_last_commit()
            .expect("Read last commit should not fail");
        assert_eq!(
            last_commit.as_ref(),
            written_commits.last(),
            "{last_commit:?}"
        );
    }

    {
        let scanned_commits = store
            .scan_commits((20..=24).into())
            .expect("Scan commits should not fail");
        assert!(scanned_commits.is_empty(), "{scanned_commits:?}");
    }

    {
        let scanned_commits = store
            .scan_commits((3..=4).into())
            .expect("Scan commits should not fail");
        assert_eq!(scanned_commits.len(), 2, "{scanned_commits:?}");
        assert_eq!(
            scanned_commits,
            vec![written_commits[2].clone(), written_commits[3].clone()]
        );
    }

    {
        let scanned_commits = store
            .scan_commits((0..=2).into())
            .expect("Scan commits should not fail");
        assert_eq!(scanned_commits.len(), 2, "{scanned_commits:?}");
        assert_eq!(
            scanned_commits,
            vec![written_commits[0].clone(), written_commits[1].clone()]
        );
    }

    {
        let scanned_commits = store
            .scan_commits((0..=4).into())
            .expect("Scan commits should not fail");
        assert_eq!(scanned_commits.len(), 4, "{scanned_commits:?}");
        assert_eq!(scanned_commits, written_commits,);
    }
}

#[rstest]
#[tokio::test]
async fn test_voting_block_headers_storage(
    #[values(new_rocksdb_teststore(true), new_mem_teststore(true))] test_store: TestStore,
) {
    let store = test_store.store();
    let context = test_store.context();

    // Create blocks with commit votes
    let voting_blocks: Vec<VerifiedBlock> = vec![
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(10, 0)
                .set_commit_votes(vec![CommitRef::new(5, CommitDigest::MIN)])
                .build(),
        ),
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(10, 1)
                .set_commit_votes(vec![CommitRef::new(5, CommitDigest::MIN)])
                .build(),
        ),
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(11, 0)
                .set_commit_votes(vec![CommitRef::new(6, CommitDigest::MIN)])
                .build(),
        ),
    ];

    let voting_headers: Vec<_> = voting_blocks
        .iter()
        .map(|b| b.verified_block_header.clone())
        .collect();

    // Write to voting storage using WriteBatch
    let write_batch = WriteBatch::default().voting_block_headers(voting_headers.clone());
    store
        .write(write_batch, context)
        .expect("Write voting block headers should not fail");

    // Read back
    let refs: Vec<_> = voting_blocks.iter().map(|b| b.reference()).collect();
    let read_headers = store
        .read_voting_block_headers(&refs)
        .expect("Read voting block headers should not fail");

    assert_eq!(read_headers.len(), 3);
    assert_eq!(read_headers[0].as_ref().unwrap(), &voting_headers[0]);
    assert_eq!(read_headers[1].as_ref().unwrap(), &voting_headers[1]);
    assert_eq!(read_headers[2].as_ref().unwrap(), &voting_headers[2]);

    // Verify NOT in regular storage (isolation)
    let regular_headers = store
        .read_verified_block_headers(&refs)
        .expect("Read verified block headers should not fail");
    assert!(regular_headers[0].is_none());
    assert!(regular_headers[1].is_none());
    assert!(regular_headers[2].is_none());

    // Test missing ref returns None
    let missing_ref = BlockRef::new(
        99,
        AuthorityIndex::new_for_test(0),
        BlockHeaderDigest::default(),
    );
    let missing = store
        .read_voting_block_headers(&[missing_ref])
        .expect("Read missing should not fail");
    assert!(missing[0].is_none());

    // Test reading subset with some missing
    let mixed_refs = vec![refs[0], missing_ref, refs[2]];
    let mixed_results = store
        .read_voting_block_headers(&mixed_refs)
        .expect("Read mixed should not fail");
    assert_eq!(mixed_results.len(), 3);
    assert!(mixed_results[0].is_some());
    assert!(mixed_results[1].is_none());
    assert!(mixed_results[2].is_some());
}

#[rstest]
#[tokio::test]
async fn test_read_highest_commit_index_with_votes(
    #[values(new_rocksdb_teststore(true), new_mem_teststore(true))] test_store: TestStore,
) {
    let store = test_store.store();
    let context = test_store.context();

    // Create blocks voting on commits at indexes 1, 2, 5, 10 (with gaps at 3, 4,
    // 6-9)
    let blocks_with_votes = [
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(5, 0)
                .set_commit_votes(vec![CommitRef::new(1, CommitDigest::MIN)])
                .build(),
        ),
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(6, 0)
                .set_commit_votes(vec![CommitRef::new(2, CommitDigest::MIN)])
                .build(),
        ),
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(10, 0)
                .set_commit_votes(vec![CommitRef::new(5, CommitDigest::MIN)])
                .build(),
        ),
        VerifiedBlock::new_for_test(
            TestBlockHeader::new(15, 0)
                .set_commit_votes(vec![CommitRef::new(10, CommitDigest::MIN)])
                .build(),
        ),
    ];

    // Write blocks (this records commit votes in the commit_votes table)
    store
        .write(
            WriteBatch::default().block_headers(
                blocks_with_votes
                    .iter()
                    .map(|b| b.verified_block_header.clone())
                    .collect(),
            ),
            context,
        )
        .expect("Write should not fail");

    // Test read_highest_commit_index_with_votes - should find highest index with
    // votes
    assert_eq!(
        store.read_highest_commit_index_with_votes(10).unwrap(),
        Some(10),
        "Should find votes at index 10"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(9).unwrap(),
        Some(5),
        "Should skip gap 6-9 and find votes at index 5"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(7).unwrap(),
        Some(5),
        "Should skip gap and find votes at index 5"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(5).unwrap(),
        Some(5),
        "Should find votes at index 5"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(4).unwrap(),
        Some(2),
        "Should skip gap 3-4 and find votes at index 2"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(3).unwrap(),
        Some(2),
        "Should skip gap and find votes at index 2"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(2).unwrap(),
        Some(2),
        "Should find votes at index 2"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(1).unwrap(),
        Some(1),
        "Should find votes at index 1"
    );
    assert_eq!(
        store.read_highest_commit_index_with_votes(0).unwrap(),
        None,
        "Should return None when no votes exist at or below index 0"
    );

    // Test with very high index
    assert_eq!(
        store.read_highest_commit_index_with_votes(1000).unwrap(),
        Some(10),
        "Should find highest votes at index 10 even when searching up to 1000"
    );
}
