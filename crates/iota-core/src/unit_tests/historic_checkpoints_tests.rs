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

/// A checkpoint whose epoch has already been expired is still filed in the
/// sequence-keyed table that is never pruned, while the digest-keyed copy and
/// the contents are skipped rather than refused.
///
/// State sync carries checkpoints of its own across a reconfiguration, so it
/// can still be inserting an epoch that the boundary has just dropped. Before
/// this it reopened the expired bucket and panicked the node.
#[tokio::test]
async fn a_checkpoint_of_an_expired_epoch_is_filed_without_its_bucket() {
    const EXPIRED_EPOCH: EpochId = 1;
    const RETAINED_EPOCH: EpochId = 3;

    let store = CheckpointStore::new_for_tests();
    // Give the store a bucket to count retention from, then expire everything
    // below the retained epoch.
    store.historic_checkpoints.ensure(RETAINED_EPOCH).unwrap();
    store.historic_checkpoints.prune(RETAINED_EPOCH, 1).unwrap();

    let full_contents = FullCheckpointContents::random_for_testing();
    let checkpoint = test_checkpoint_with_contents(EXPIRED_EPOCH, 5, &full_contents);

    // Neither write fails, and neither reopens the expired epoch.
    store
        .insert_checkpoint_contents(&checkpoint, full_contents.checkpoint_contents())
        .unwrap();
    store.insert_certified_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        store.historic_checkpoints.earliest_bucket_epoch(),
        Some(RETAINED_EPOCH)
    );

    // The summary is still reachable by sequence number, which is what state
    // sync's chain of trust reads.
    assert_eq!(
        store
            .get_checkpoint_by_sequence_number(5)
            .unwrap()
            .map(|summary| *summary.digest()),
        Some(*checkpoint.digest())
    );
    // And not by digest, since the epoch that would hold it is gone.
    assert!(
        store
            .get_checkpoint_by_digest(checkpoint.digest())
            .unwrap()
            .is_none()
    );
}
