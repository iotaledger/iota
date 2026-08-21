// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{committee::EpochId, messages_checkpoint::FullCheckpointContents};
use typed_store::traits::Map;

use crate::checkpoints::{CheckpointStore, test_checkpoint_with_contents};

/// The epoch the checkpoint below is certified in.
const CHECKPOINT_EPOCH: EpochId = 2;

/// The sequence number of that checkpoint.
const CHECKPOINT_SEQUENCE: u64 = 5;

/// A certified checkpoint's digest-keyed summary and its contents are written
/// to, and read back from, the bucket of the epoch that closed it, while the
/// sequence-keyed summary stays in the flat table that is never pruned.
#[tokio::test]
async fn a_checkpoint_and_its_contents_are_read_from_their_epoch_bucket() {
    let store = CheckpointStore::new_for_tests();
    let full_contents = FullCheckpointContents::random_for_testing();
    let checkpoint =
        test_checkpoint_with_contents(CHECKPOINT_EPOCH, CHECKPOINT_SEQUENCE, &full_contents);
    let contents = full_contents.checkpoint_contents();
    let contents_digest = checkpoint.contents_digest;

    store
        .insert_checkpoint_contents(&checkpoint, contents)
        .unwrap();
    store.insert_certified_checkpoint(&checkpoint).unwrap();

    // The sequence-keyed summary is in the flat table, which is never bucketed.
    assert_eq!(
        store
            .tables
            .certified_checkpoints
            .get(&CHECKPOINT_SEQUENCE)
            .unwrap()
            .map(|summary| *summary.inner().digest()),
        Some(*checkpoint.digest())
    );

    // Both writes went to the bucket of the checkpoint's own epoch, and to no
    // other: this is the only bucket the store holds.
    assert_eq!(
        store.historic_checkpoints.earliest_bucket_epoch(),
        Some(CHECKPOINT_EPOCH)
    );
    let bucket = store.historic_checkpoints.ensure(CHECKPOINT_EPOCH).unwrap();
    assert_eq!(
        bucket
            .checkpoint_by_digest
            .get(checkpoint.digest())
            .unwrap()
            .map(|summary| *summary.inner().digest()),
        Some(*checkpoint.digest())
    );
    assert_eq!(
        bucket
            .checkpoint_content
            .get(&contents_digest)
            .unwrap()
            .map(|contents| contents.digest()),
        Some(contents_digest)
    );

    // Neither flat table holds them.
    assert!(
        store
            .tables
            .checkpoint_by_digest
            .get(checkpoint.digest())
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .tables
            .checkpoint_content
            .get(&contents_digest)
            .unwrap()
            .is_none()
    );

    // The store's own readers resolve both through the bucket.
    assert_eq!(
        store
            .get_checkpoint_by_digest(checkpoint.digest())
            .unwrap()
            .map(|summary| *summary.digest()),
        Some(*checkpoint.digest())
    );
    assert_eq!(
        store
            .get_checkpoint_contents(&contents_digest)
            .unwrap()
            .map(|contents| contents.digest()),
        Some(contents_digest)
    );
    assert_eq!(
        store
            .multi_get_checkpoint_content(&[contents_digest])
            .unwrap()
            .into_iter()
            .map(|contents| contents.map(|contents| contents.digest()))
            .collect::<Vec<_>>(),
        vec![Some(contents_digest)]
    );
}

/// Rows written before this history was bucketed are still readable. A
/// database written by an earlier binary has them only in the flat tables,
/// and the first thing a node does on start is look the genesis checkpoint up
/// by digest: were that lookup to miss, the start would take the genesis
/// checkpoint for absent and reset its synced and verified watermarks to zero.
#[tokio::test]
async fn pre_bucket_rows_are_still_read() {
    let store = CheckpointStore::new_for_tests();
    let full_contents = FullCheckpointContents::random_for_testing();
    let checkpoint =
        test_checkpoint_with_contents(CHECKPOINT_EPOCH, CHECKPOINT_SEQUENCE, &full_contents);
    let contents = full_contents.checkpoint_contents();
    let contents_digest = checkpoint.contents_digest;

    // Written the way the binary before the buckets wrote them.
    store
        .tables
        .checkpoint_content
        .insert(&contents_digest, &contents)
        .unwrap();
    store
        .tables
        .checkpoint_by_digest
        .insert(checkpoint.digest(), checkpoint.serializable_ref())
        .unwrap();

    // No bucket holds anything, so every read has to fall through to them.
    assert_eq!(store.historic_checkpoints.earliest_bucket_epoch(), None);
    assert_eq!(
        store
            .get_checkpoint_by_digest(checkpoint.digest())
            .unwrap()
            .map(|summary| *summary.digest()),
        Some(*checkpoint.digest())
    );
    assert_eq!(
        store
            .get_checkpoint_contents(&contents_digest)
            .unwrap()
            .map(|contents| contents.digest()),
        Some(contents_digest)
    );
    assert_eq!(
        store
            .multi_get_checkpoint_content(&[contents_digest])
            .unwrap()
            .into_iter()
            .map(|contents| contents.map(|contents| contents.digest()))
            .collect::<Vec<_>>(),
        vec![Some(contents_digest)]
    );
}
