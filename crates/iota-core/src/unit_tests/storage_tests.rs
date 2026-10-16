// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::RangeInclusive, sync::Arc, time::Duration};

use iota_config::p2p::StateSyncConfig;
use iota_swarm_config::test_utils::CommitteeFixture;
use iota_types::{
    messages_checkpoint::{
        CheckpointSequenceNumber, VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    storage::WriteStore,
};
use prometheus_filtered::Registry;
use tokio::{sync::broadcast::error::TryRecvError, task::JoinHandle, time::timeout};

use crate::{
    authority::test_authority_builder::TestAuthorityBuilder, checkpoints::CheckpointStore,
    execution_cache::build_execution_cache_from_env, storage::RocksDbStore,
};

/// The longest time a test waits for state sync or for a waiter. The waiters
/// in these tests have no fallback, so a lost wake-up never resolves.
const WAIT_TIMEOUT: Duration = Duration::from_secs(30);

async fn wait_until(what: &str, condition: impl Fn() -> bool) {
    timeout(WAIT_TIMEOUT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

/// A `RocksDbStore` on a new checkpoint store, with checkpoint 0 synced, and
/// the random checkpoints after it with their contents, which are not stored.
struct Stores {
    checkpoint_store: Arc<CheckpointStore>,
    store: RocksDbStore,
    checkpoints: Vec<VerifiedCheckpoint>,
    contents: Vec<VerifiedCheckpointContents>,
}

impl Stores {
    /// `count` is the number of checkpoints, checkpoint 0 included.
    async fn new_with_checkpoint_0_synced(count: usize) -> Self {
        telemetry_subscribers::init_for_testing();
        let state = TestAuthorityBuilder::new().build().await;
        let checkpoint_store = CheckpointStore::new_for_tests();
        let store = RocksDbStore::new(
            build_execution_cache_from_env(&Registry::new(), &state.database_for_testing()),
            state.committee_store().clone(),
            checkpoint_store.clone(),
        );
        let committee =
            CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
        let (checkpoints, contents, _, _) = committee.make_random_checkpoints(count, None);

        checkpoint_store
            .insert_verified_checkpoint(&checkpoints[0])
            .unwrap();
        store
            .try_insert_checkpoint_contents(&checkpoints[0], contents[0].clone())
            .unwrap();
        store
            .try_update_highest_synced_checkpoint(&checkpoints[0])
            .unwrap();
        Self {
            checkpoint_store,
            store,
            checkpoints,
            contents,
        }
    }

    fn highest_synced(&self) -> Option<CheckpointSequenceNumber> {
        self.checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap()
    }

    fn num_waiters(&self) -> usize {
        self.checkpoint_store
            .num_synced_checkpoint_waiters_for_testing()
    }

    /// Spawns a waiter for checkpoint `seq` to be synced, like the one the
    /// checkpoint executor parks, and returns once the waiter is registered.
    async fn spawn_waiter(&self, seq: CheckpointSequenceNumber) -> JoinHandle<VerifiedCheckpoint> {
        let registered = self.num_waiters() + 1;
        let waiter = tokio::spawn({
            let checkpoint_store = self.checkpoint_store.clone();
            async move { checkpoint_store.notify_read_synced_checkpoint(seq).await }
        });
        // A waiter that registers after the watermark passed `seq` returns at
        // once, whether or not a write released it. On the single-threaded
        // test runtime, a registered waiter has also read the watermark.
        wait_until("the waiter to register", || {
            self.num_waiters() == registered
        })
        .await;
        waiter
    }

    /// The checkpoints `seqs` with their contents, as archive sync writes them.
    fn run_of_checkpoints(
        &self,
        seqs: RangeInclusive<usize>,
    ) -> Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)> {
        seqs.map(|seq| (self.checkpoints[seq].clone(), self.contents[seq].clone()))
            .collect()
    }
}

/// State sync on a `RocksDbStore`, with checkpoint 0 synced, checkpoint 1
/// verified but not synced, and no peers. It holds a waiter for checkpoint 1.
struct Node {
    stores: Stores,
    state_sync: iota_network::state_sync::Handle,
    waiter: JoinHandle<VerifiedCheckpoint>,
}

impl Node {
    /// Starts state sync on a new `RocksDbStore` with checkpoint 1 verified,
    /// and registers a waiter for checkpoint 1.
    async fn new_with_checkpoint_1_verified() -> Self {
        let stores = Stores::new_with_checkpoint_0_synced(3).await;
        stores
            .checkpoint_store
            .insert_verified_checkpoint(&stores.checkpoints[1])
            .unwrap();

        let (state_sync, server) = iota_network::state_sync::Builder::new()
            .store(stores.store.clone())
            .config(StateSyncConfig {
                wait_interval_when_no_peer_to_sync_content_ms: Some(50),
                ..Default::default()
            })
            .build();
        let network = anemo::Network::bind("localhost:0")
            .private_key([7; 32])
            .server_name("test")
            .start(anemo::Router::new().add_rpc_service(server))
            .unwrap();
        let state_sync = state_sync.start(network);
        let waiter = stores.spawn_waiter(1).await;
        Self {
            stores,
            state_sync,
            waiter,
        }
    }

    /// The checkpoint builder stores the contents of checkpoint 2, and the
    /// aggregator certifies checkpoint 2 and sends it to state sync. Returns
    /// once state sync has verified checkpoint 2.
    async fn send_checkpoint_2_from_consensus(&self) {
        let checkpoint = &self.stores.checkpoints[2];
        self.stores
            .store
            .try_insert_checkpoint_contents(checkpoint, self.stores.contents[2].clone())
            .unwrap();
        self.stores
            .checkpoint_store
            .insert_certified_checkpoint(checkpoint)
            .unwrap();
        self.state_sync.send_checkpoint(checkpoint.clone()).await;
        wait_until("checkpoint 2 to be verified", || {
            self.stores
                .checkpoint_store
                .get_highest_verified_checkpoint_seq_number()
                .unwrap()
                == Some(2)
        })
        .await;
    }

    /// Stores the full contents of checkpoint 1, as state sync does after it
    /// downloads them from a peer.
    fn download_checkpoint_1_contents(&self) {
        self.stores
            .store
            .try_insert_checkpoint_contents(
                &self.stores.checkpoints[1],
                self.stores.contents[1].clone(),
            )
            .unwrap();
    }

    async fn assert_waiter_released(&mut self, failure: &str) {
        assert_eq!(
            self.stores.num_waiters(),
            0,
            "{failure}; the highest synced checkpoint is {:?}",
            self.stores.highest_synced(),
        );
        let released = timeout(WAIT_TIMEOUT, &mut self.waiter)
            .await
            .expect("a released waiter returns")
            .unwrap();
        assert_eq!(released.sequence_number(), 1);
    }
}

/// Checkpoint 1 is verified but not synced, and checkpoint 2 comes from
/// consensus. The write for checkpoint 2 must not move the synced watermark
/// past checkpoint 1, which state sync has not marked synced yet. The test
/// checks that the watermark stays at checkpoint 0 and that state sync sends
/// no synced checkpoint to its subscribers. When state sync gets the contents
/// of checkpoint 1, it marks checkpoints 1 and 2 synced in this order, and the
/// waiter for checkpoint 1 is released.
///
/// No contents of checkpoint 1 are in the store. A validator does not get into
/// this state. Only a validator gets checkpoints from consensus, and its
/// checkpoint builder stores the contents of checkpoint 1 before checkpoint 2
/// can be certified.
#[tokio::test]
async fn checkpoint_from_consensus_marked_synced_only_after_previous_checkpoint_synced() {
    let mut node = Node::new_with_checkpoint_1_verified().await;
    let mut synced_checkpoints = node.state_sync.subscribe_to_synced_checkpoints();
    node.send_checkpoint_2_from_consensus().await;
    // Lets state sync retry checkpoint 1 before the test checks the watermark.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        node.stores.highest_synced(),
        Some(0),
        "the synced watermark passed checkpoint 1, which is not synced",
    );
    assert_eq!(
        node.stores.num_waiters(),
        1,
        "the waiter for checkpoint 1 was released before checkpoint 1 was synced"
    );
    assert_eq!(
        synced_checkpoints
            .try_recv()
            .map(|checkpoint| checkpoint.sequence_number()),
        Err(TryRecvError::Empty),
        "state sync sent a synced checkpoint to its subscribers before checkpoint 1 was synced"
    );

    node.download_checkpoint_1_contents();
    for seq in [1, 2] {
        let checkpoint = timeout(WAIT_TIMEOUT, synced_checkpoints.recv())
            .await
            .unwrap_or_else(|_| {
                panic!("timed out waiting for state sync to send synced checkpoint {seq}")
            })
            .unwrap();
        assert_eq!(
            checkpoint.sequence_number(),
            seq,
            "state sync sent the synced checkpoints to its subscribers out of order"
        );
    }
    assert_eq!(
        node.stores.highest_synced(),
        Some(2),
        "state sync sent synced checkpoints 1 and 2, but the synced watermark did not move to 2"
    );
    node.assert_waiter_released(
        "the waiter for checkpoint 1 is not released after checkpoint 1 is synced",
    )
    .await;
}

/// Checkpoint 1 is verified but not synced, its contents are in the store, and
/// checkpoint 2 comes from consensus. A validator can be in this state. Its
/// checkpoint builder stores the contents of checkpoint 1 before checkpoint 2
/// can be certified, and state sync may not have marked checkpoint 1 synced
/// yet. The test checks that the write that moves the synced watermark past
/// checkpoint 1 releases the waiter for checkpoint 1. It does not check how
/// long the waiter waits.
#[tokio::test]
async fn waiter_for_previous_checkpoint_released_after_checkpoint_from_consensus() {
    let mut node = Node::new_with_checkpoint_1_verified().await;
    // The test stores only the contents of checkpoint 1, not its transactions
    // and effects, which a validator has executed by then. Without them state
    // sync cannot read the full contents of checkpoint 1 from the store, as if
    // it were still getting them from a peer.
    node.stores
        .checkpoint_store
        .insert_checkpoint_contents(node.stores.contents[1].clone().into_checkpoint_contents())
        .unwrap();
    node.send_checkpoint_2_from_consensus().await;

    node.download_checkpoint_1_contents();
    wait_until("checkpoints 1 and 2 to be synced", || {
        node.stores.highest_synced() == Some(2)
    })
    .await;
    node.assert_waiter_released(
        "the synced watermark passed checkpoint 1 without releasing its waiter",
    )
    .await;
}

/// Archive sync marks runs of checkpoints synced. `RocksDbStore` marks a run
/// synced only if the checkpoint before its first checkpoint is synced. The
/// write releases the waiter of each checkpoint that the synced watermark
/// passes.
#[tokio::test]
async fn run_of_checkpoints_marked_synced_only_after_checkpoint_before_run_synced() {
    let stores = Stores::new_with_checkpoint_0_synced(5).await;
    let mut waiters = Vec::new();
    for seq in 1..=3 {
        waiters.push(stores.spawn_waiter(seq).await);
    }

    stores
        .store
        .try_insert_synced_checkpoints(stores.run_of_checkpoints(2..=3))
        .unwrap();
    assert_eq!(
        stores.highest_synced(),
        Some(0),
        "a run that starts after checkpoint 1 moved the synced watermark"
    );
    assert_eq!(
        stores.num_waiters(),
        3,
        "a run that did not move the synced watermark released a waiter"
    );

    stores
        .store
        .try_insert_synced_checkpoints(stores.run_of_checkpoints(1..=3))
        .unwrap();
    assert_eq!(
        stores.highest_synced(),
        Some(3),
        "a run that starts right after the synced watermark did not move it"
    );
    assert_eq!(
        stores.num_waiters(),
        0,
        "the run did not release the waiter of each checkpoint in it"
    );
    for (seq, waiter) in (1..=3).zip(waiters) {
        let released = timeout(WAIT_TIMEOUT, waiter)
            .await
            .expect("a released waiter returns")
            .unwrap();
        assert_eq!(released.sequence_number(), seq);
    }

    let waiter = stores.spawn_waiter(4).await;
    stores
        .store
        .try_insert_synced_checkpoints(stores.run_of_checkpoints(2..=4))
        .unwrap();
    assert_eq!(
        stores.highest_synced(),
        Some(4),
        "a run that starts at or below the synced watermark did not move it"
    );
    assert_eq!(
        stores.num_waiters(),
        0,
        "the run did not release the waiter for checkpoint 4"
    );
    let released = timeout(WAIT_TIMEOUT, waiter)
        .await
        .expect("a released waiter returns")
        .unwrap();
    assert_eq!(released.sequence_number(), 4);
}
