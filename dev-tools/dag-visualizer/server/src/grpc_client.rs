// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reconnecting gRPC client that streams data from the validator,
//! stores it in RocksDB, and broadcasts to local WebSocket clients.

use std::sync::Arc;

use dag_visualizer_proto::dag_visualizer::{
    DagEvent, GetCommitteeRequest, GetStatusRequest, LeaderStatus, StreamDagEventsRequest,
    dag_event::Event, dag_visualizer_service_client::DagVisualizerServiceClient,
};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::{
    storage::{
        StorageManager, StoredBlock, StoredCommittee, StoredLeader, StoredStatus, StoredValidator,
    },
    types::*,
};

/// Try to convert a byte slice to a 32-byte digest array.
/// Logs a warning and returns `None` if the slice is not exactly 32 bytes.
fn try_digest(bytes: &[u8], context: &str) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok().or_else(|| {
        if !bytes.is_empty() {
            warn!(
                "Malformed {context} digest: expected 32 bytes, got {}",
                bytes.len()
            );
        }
        None
    })
}

/// Try to convert a proto u32 author field to u8.
/// Returns `None` with a warning if the value exceeds `u8::MAX`.
fn author_u8(value: u32) -> Option<u8> {
    u8::try_from(value).ok().or_else(|| {
        warn!("author index {value} exceeds u8::MAX (255), skipping entry");
        None
    })
}

/// Run the gRPC client loop with reconnection.
///
/// This connects to the validator, fetches committee info, then streams
/// events. Events are stored in RocksDB and forwarded to the local
/// broadcast channel for WebSocket clients.
pub async fn run_grpc_client(
    validator_addr: String,
    storage: Arc<StorageManager>,
    event_tx: broadcast::Sender<DagVisualizerEvent>,
) {
    let mut backoff_ms = 100u64;
    let max_backoff_ms = 10_000u64;

    loop {
        info!("Connecting to validator gRPC at {validator_addr}...");

        match connect_and_stream(&validator_addr, &storage, &event_tx).await {
            Ok(()) => {
                info!("gRPC stream ended gracefully");
                // Reset backoff on successful (graceful) connection
                backoff_ms = 100;
            }
            Err(e) => {
                warn!("gRPC connection error: {e}");
            }
        }

        // Exponential backoff
        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
    }
}

async fn connect_and_stream(
    addr: &str,
    storage: &Arc<StorageManager>,
    event_tx: &broadcast::Sender<DagVisualizerEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let channel = tonic::transport::Channel::from_shared(addr.to_string())?
        .http2_keep_alive_interval(std::time::Duration::from_secs(5))
        .keep_alive_timeout(std::time::Duration::from_secs(10))
        .connect()
        .await?;
    let mut client = DagVisualizerServiceClient::new(channel);

    // 1. Get committee → detect epoch, open epoch DB
    let committee_resp = client
        .get_committee(GetCommitteeRequest {})
        .await?
        .into_inner();
    let epoch = committee_resp.epoch;
    info!("Connected to validator, epoch {epoch}");

    let epoch_store = storage.get_or_create_epoch(epoch);

    // Store committee
    let stored_committee = StoredCommittee {
        epoch,
        total_stake: committee_resp.total_stake,
        quorum_threshold: committee_resp.quorum_threshold,
        validators: committee_resp
            .validators
            .iter()
            .filter_map(|v| {
                Some(StoredValidator {
                    index: author_u8(v.index)?,
                    hostname: v.hostname.clone(),
                    stake: v.stake,
                })
            })
            .collect(),
    };
    epoch_store.set_committee(&stored_committee);

    // 2. Get initial status
    let status_resp = client.get_status(GetStatusRequest {}).await?.into_inner();
    epoch_store.set_status(&StoredStatus {
        highest_accepted_round: status_resp.highest_accepted_round,
        last_commit_index: status_resp.last_commit_index,
        last_commit_round: status_resp.last_commit_round,
        num_authorities: status_resp.num_authorities,
    });

    // 3. Stream live events
    let mut stream = client
        .stream_dag_events(StreamDagEventsRequest {})
        .await?
        .into_inner();

    info!("Streaming live DAG events...");

    let mut cached = CachedState {
        last_round: epoch_store.get_last_round(),
        first_round: epoch_store.get_first_round(),
        status: epoch_store.get_status(),
    };

    let timeout_duration = std::time::Duration::from_secs(30);
    loop {
        match tokio::time::timeout(timeout_duration, stream.message()).await {
            Ok(Ok(Some(event))) => {
                process_event(&event, &epoch_store, event_tx, &mut cached);
            }
            Ok(Ok(None)) => {
                info!("gRPC stream ended (server closed)");
                break;
            }
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => {
                warn!("gRPC stream idle for {timeout_duration:?}, reconnecting");
                break;
            }
        }
    }

    Ok(())
}

/// In-memory cache to avoid redundant RocksDB reads on every event.
pub(crate) struct CachedState {
    pub last_round: u32,
    pub first_round: u32,
    pub status: Option<StoredStatus>,
}

pub(crate) fn process_event(
    event: &DagEvent,
    store: &Arc<crate::storage::EpochStore>,
    event_tx: &broadcast::Sender<DagVisualizerEvent>,
    cached: &mut CachedState,
) {
    match &event.event {
        Some(Event::BlockAccepted(block)) => {
            let digest = try_digest(&block.digest, "block").unwrap_or([0u8; 32]);
            let Some(author) = author_u8(block.author) else {
                return;
            };

            // Filter out ancestors with invalid author indices
            let ancestors: Vec<(u32, u8)> = block
                .ancestors
                .iter()
                .filter_map(|a| Some((a.round, author_u8(a.author)?)))
                .collect();

            let stored = StoredBlock {
                round: block.round,
                author,
                digest,
                timestamp_ms: block.timestamp_ms,
                ancestors: ancestors.clone(),
                acknowledgments: block
                    .acknowledgments
                    .iter()
                    .filter_map(|a| {
                        let d = try_digest(&a.digest, "acknowledgment")?;
                        Some((a.round, author_u8(a.author)?, d))
                    })
                    .collect(),
            };
            store.insert_block(&stored);

            // Track first round seen in this epoch
            if cached.first_round == 0 {
                cached.first_round = block.round;
                store.set_first_round(block.round);
            }

            if block.round > cached.last_round {
                cached.last_round = block.round;
                store.set_last_round(block.round);
            }

            // Forward to WebSocket clients
            let msg_block = DagBlockMessage {
                round: block.round,
                author,
                digest: short_digest(&hex::encode(digest)),
                timestamp_ms: block.timestamp_ms,
                ancestors: block
                    .ancestors
                    .iter()
                    .filter_map(|a| {
                        let d = try_digest(&a.digest, "ancestor").unwrap_or([0u8; 32]);
                        Some(BlockRefMessage {
                            round: a.round,
                            author: author_u8(a.author)?,
                            digest: short_digest(&hex::encode(d)),
                        })
                    })
                    .collect(),
                acknowledgments: block
                    .acknowledgments
                    .iter()
                    .filter_map(|a| {
                        let d = try_digest(&a.digest, "acknowledgment").unwrap_or([0u8; 32]);
                        Some(BlockRefMessage {
                            round: a.round,
                            author: author_u8(a.author)?,
                            digest: short_digest(&hex::encode(d)),
                        })
                    })
                    .collect(),
            };
            let _ = event_tx.send(DagVisualizerEvent::BlockAccepted(msg_block));
        }
        Some(Event::LeaderDecided(leader)) => {
            let digest: Option<[u8; 32]> = if leader.block_digest.is_empty() {
                None
            } else {
                try_digest(&leader.block_digest, "leader")
            };
            let Some(leader_authority) = author_u8(leader.leader_authority) else {
                return;
            };
            let Some(proto_status) = LeaderStatus::try_from(leader.status).ok() else {
                warn!(
                    "unknown leader status {}, skipping entry",
                    leader.status
                );
                return;
            };
            let status = match proto_status {
                LeaderStatus::Committed => LEADER_COMMITTED,
                LeaderStatus::Skipped => LEADER_SKIPPED,
            };

            let stored = StoredLeader {
                wave: leader.wave,
                leader_round: leader.leader_round,
                leader_authority,
                status,
                block_digest: digest,
            };
            store.insert_leader(&stored);

            // Update cached status with commit info
            if status == LEADER_COMMITTED {
                if let Some(ref mut cached_status) = cached.status {
                    if leader.leader_round > cached_status.last_commit_round {
                        cached_status.last_commit_round = leader.leader_round;
                        cached_status.last_commit_index += 1;
                        store.set_status(cached_status);
                    }
                }
            }

            let msg_leader = LeaderInfoMessage {
                wave: leader.wave,
                leader_round: leader.leader_round,
                leader_authority,
                status,
                block_digest: digest.map(|d| short_digest(&hex::encode(d))),
            };
            let _ = event_tx.send(DagVisualizerEvent::LeaderDecided(msg_leader));
        }
        Some(Event::RoundAdvanced(round_event)) => {
            // Update cached status
            if let Some(ref mut cached_status) = cached.status {
                if round_event.round > cached_status.highest_accepted_round {
                    cached_status.highest_accepted_round = round_event.round;
                    store.set_status(cached_status);
                }
            }

            let _ = event_tx.send(DagVisualizerEvent::RoundAdvanced {
                round: round_event.round,
            });
        }
        None => {
            error!("Received DagEvent with no event payload");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dag_visualizer_proto::dag_visualizer::{
        BlockAcceptedEvent, BlockRefProto, DagEvent, LeaderDecidedEvent, LeaderStatus,
        RoundAdvancedEvent, dag_event::Event,
    };
    use tokio::sync::broadcast;

    use super::{CachedState, process_event};
    use crate::{
        storage::{EpochStore, StoredStatus},
        types::DagVisualizerEvent,
    };

    fn open_test_store(dir: &std::path::Path) -> Arc<EpochStore> {
        Arc::new(EpochStore::open_for_test(dir))
    }

    fn default_cached() -> CachedState {
        CachedState {
            last_round: 0,
            first_round: 0,
            status: None,
        }
    }

    fn cached_with_status(status: StoredStatus) -> CachedState {
        CachedState {
            last_round: 0,
            first_round: 0,
            status: Some(status),
        }
    }

    #[tokio::test]
    async fn process_block_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let (tx, mut rx) = broadcast::channel::<DagVisualizerEvent>(16);
        let mut cached = default_cached();

        let event = DagEvent {
            event: Some(Event::BlockAccepted(BlockAcceptedEvent {
                round: 5,
                author: 2,
                digest: vec![0xABu8; 32],
                timestamp_ms: 5000,
                ancestors: vec![BlockRefProto {
                    round: 4,
                    author: 1,
                    digest: vec![0xCDu8; 32],
                }],
                acknowledgments: vec![BlockRefProto {
                    round: 3,
                    author: 0,
                    digest: vec![0xEEu8; 32],
                }],
            })),
        };

        process_event(&event, &store, &tx, &mut cached);

        // Verify stored in RocksDB
        let blocks = store.get_blocks_in_range(5, 5);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].round, 5);
        assert_eq!(blocks[0].author, 2);
        assert_eq!(blocks[0].timestamp_ms, 5000);
        assert_eq!(blocks[0].ancestors, vec![(4, 1)]);
        assert_eq!(blocks[0].acknowledgments.len(), 1);
        assert_eq!(blocks[0].acknowledgments[0].0, 3);
        assert_eq!(blocks[0].acknowledgments[0].1, 0);

        // Verify last_round and first_round updated
        assert_eq!(store.get_last_round(), 5);
        assert_eq!(store.get_first_round(), 5);
        assert_eq!(cached.last_round, 5);
        assert_eq!(cached.first_round, 5);

        // Verify broadcast event
        let received = rx.try_recv().unwrap();
        match received {
            DagVisualizerEvent::BlockAccepted(b) => {
                assert_eq!(b.round, 5);
                assert_eq!(b.author, 2);
            }
            _ => panic!("Expected BlockAccepted event"),
        }
    }

    #[tokio::test]
    async fn process_leader_committed() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let (tx, mut rx) = broadcast::channel::<DagVisualizerEvent>(16);

        // Set initial status so the update logic works
        let initial_status = StoredStatus {
            highest_accepted_round: 10,
            last_commit_index: 0,
            last_commit_round: 0,
            num_authorities: 4,
        };
        store.set_status(&initial_status);
        let mut cached = cached_with_status(initial_status);

        let event = DagEvent {
            event: Some(Event::LeaderDecided(LeaderDecidedEvent {
                wave: 3,
                leader_round: 6,
                leader_authority: 1,
                status: LeaderStatus::Committed.into(),
                block_digest: vec![0xFFu8; 32],
            })),
        };
        process_event(&event, &store, &tx, &mut cached);

        // Verify stored
        let leaders = store.get_leaders_in_range(6, 6);
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].wave, 3);
        assert_eq!(leaders[0].status, 0);

        // Verify status updated
        let status = store.get_status().unwrap();
        assert_eq!(status.last_commit_round, 6);
        assert_eq!(status.last_commit_index, 1);

        // Verify broadcast
        let received = rx.try_recv().unwrap();
        match received {
            DagVisualizerEvent::LeaderDecided(l) => {
                assert_eq!(l.leader_round, 6);
                assert_eq!(l.status, 0);
            }
            _ => panic!("Expected LeaderDecided event"),
        }
    }

    #[tokio::test]
    async fn process_leader_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let (tx, _rx) = broadcast::channel::<DagVisualizerEvent>(16);

        let initial_status = StoredStatus {
            highest_accepted_round: 10,
            last_commit_index: 0,
            last_commit_round: 0,
            num_authorities: 4,
        };
        store.set_status(&initial_status);
        let mut cached = cached_with_status(initial_status);

        let event = DagEvent {
            event: Some(Event::LeaderDecided(LeaderDecidedEvent {
                wave: 3,
                leader_round: 6,
                leader_authority: 1,
                status: LeaderStatus::Skipped.into(),
                block_digest: vec![],
            })),
        };
        process_event(&event, &store, &tx, &mut cached);

        // Verify stored
        let leaders = store.get_leaders_in_range(6, 6);
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].status, 1);

        // Verify status NOT updated (last_commit_round stays 0)
        let status = store.get_status().unwrap();
        assert_eq!(status.last_commit_round, 0);
    }

    #[tokio::test]
    async fn process_round_advanced() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let (tx, mut rx) = broadcast::channel::<DagVisualizerEvent>(16);

        let initial_status = StoredStatus {
            highest_accepted_round: 10,
            last_commit_index: 0,
            last_commit_round: 0,
            num_authorities: 4,
        };
        store.set_status(&initial_status);
        let mut cached = cached_with_status(initial_status);

        let event = DagEvent {
            event: Some(Event::RoundAdvanced(RoundAdvancedEvent { round: 50 })),
        };
        process_event(&event, &store, &tx, &mut cached);

        // Verify status updated
        let status = store.get_status().unwrap();
        assert_eq!(status.highest_accepted_round, 50);

        // Verify broadcast
        let received = rx.try_recv().unwrap();
        match received {
            DagVisualizerEvent::RoundAdvanced { round } => assert_eq!(round, 50),
            _ => panic!("Expected RoundAdvanced event"),
        }
    }

    #[tokio::test]
    async fn last_round_only_increases() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let (tx, _rx) = broadcast::channel::<DagVisualizerEvent>(16);
        let mut cached = default_cached();

        let block_10 = DagEvent {
            event: Some(Event::BlockAccepted(BlockAcceptedEvent {
                round: 10,
                author: 0,
                digest: vec![0u8; 32],
                timestamp_ms: 10_000,
                ancestors: vec![],
                acknowledgments: vec![],
            })),
        };
        process_event(&block_10, &store, &tx, &mut cached);
        assert_eq!(store.get_last_round(), 10);

        let block_5 = DagEvent {
            event: Some(Event::BlockAccepted(BlockAcceptedEvent {
                round: 5,
                author: 0,
                digest: vec![1u8; 32],
                timestamp_ms: 5000,
                ancestors: vec![],
                acknowledgments: vec![],
            })),
        };
        process_event(&block_5, &store, &tx, &mut cached);
        assert_eq!(store.get_last_round(), 10); // NOT downgraded
        assert_eq!(cached.first_round, 10); // first_round stays at initial value
    }
}
