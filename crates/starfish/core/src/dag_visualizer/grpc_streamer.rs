// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC streaming endpoint for the DAG visualizer.
//! Binds to localhost only (debugging tool, not exposed externally).

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use dag_visualizer_proto::dag_visualizer::{
    BlockAcceptedEvent, BlockRefProto, CommitteeResponse, DagEvent, GetCommitteeRequest,
    GetStatusRequest, LeaderDecidedEvent, LeaderStatus, RoundAdvancedEvent, StatusResponse,
    StreamDagEventsRequest, ValidatorInfo,
    dag_event::Event,
    dag_visualizer_service_server::{DagVisualizerService, DagVisualizerServiceServer},
};
use parking_lot::RwLock;
use tokio::sync::broadcast;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::{
    block_header::{BlockRef, Slot},
    commit::WAVE_LENGTH,
    context::Context,
    dag_state::DagState,
};

/// Default capacity for the DAG visualizer broadcast channel.
/// Shared between the validator (authority_node.rs) and gRPC server.
pub(crate) const DAG_VISUALIZER_BROADCAST_CAPACITY: usize = 4096;

/// Maximum number of concurrent gRPC stream connections.
const MAX_CONCURRENT_STREAMS: usize = 1;

/// Compute the wave number for a given round.
/// Centralised here to avoid duplicating the `(round - 1) / WAVE_LENGTH`
/// formula across `dag_state.rs` and `grpc_streamer.rs`.
fn wave_for_round(round: u32) -> u32 {
    if round > 0 {
        (round - 1) / WAVE_LENGTH
    } else {
        0
    }
}

/// Events broadcast on the internal channel.
/// These wrap internal types directly — conversion to proto happens on the
/// receiver side in [`convert_event_to_proto`].
///
/// `ancestors` and `acknowledgments` are wrapped in `Arc` so that broadcast
/// clones are cheap (reference-counted pointer copy instead of deep Vec clone).
#[derive(Clone, Debug)]
pub(crate) enum DagVisualizerEvent {
    BlockAccepted {
        block_ref: BlockRef,
        timestamp_ms: u64,
        ancestors: Arc<[BlockRef]>,
        acknowledgments: Arc<[BlockRef]>,
    },
    LeaderCommitted(BlockRef),
    LeaderSkipped(Slot),
    RoundAdvanced {
        round: u32,
    },
}

/// Shared state for the gRPC service implementation.
struct DagVisualizerServiceImpl {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    event_tx: broadcast::Sender<DagVisualizerEvent>,
    active_streams: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl DagVisualizerService for DagVisualizerServiceImpl {
    async fn get_committee(
        &self,
        _request: Request<GetCommitteeRequest>,
    ) -> Result<Response<CommitteeResponse>, Status> {
        let committee = &self.context.committee;
        let validators = committee
            .authorities()
            .map(|(idx, auth)| ValidatorInfo {
                index: idx.value() as u32,
                hostname: auth.hostname.clone(),
                stake: auth.stake,
            })
            .collect();

        Ok(Response::new(CommitteeResponse {
            epoch: committee.epoch(),
            total_stake: committee.total_stake(),
            quorum_threshold: committee.quorum_threshold(),
            validators,
        }))
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let state = self.dag_state.read();
        let last_committed_rounds = state.last_committed_rounds();
        let last_commit_round = last_committed_rounds.iter().copied().max().unwrap_or(0);
        Ok(Response::new(StatusResponse {
            highest_accepted_round: state.highest_accepted_round(),
            last_commit_index: state.last_commit_index(),
            last_commit_round,
            num_authorities: self.context.committee.size() as u32,
        }))
    }

    type StreamDagEventsStream = ReceiverStream<Result<DagEvent, Status>>;

    async fn stream_dag_events(
        &self,
        _request: Request<StreamDagEventsRequest>,
    ) -> Result<Response<Self::StreamDagEventsStream>, Status> {
        loop {
            let current = self.active_streams.load(Ordering::Acquire);
            if current >= MAX_CONCURRENT_STREAMS {
                return Err(Status::resource_exhausted("too many concurrent streams"));
            }
            if self
                .active_streams
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        let (tx, rx) = tokio::sync::mpsc::channel(4096);
        let mut event_rx = self.event_tx.subscribe();
        let active_streams = self.active_streams.clone();

        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let dag_event = convert_event_to_proto(event);
                        if tx.send(Ok(dag_event)).await.is_err() {
                            break; // Client disconnected
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("DAG visualizer gRPC stream lagged, skipped {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            active_streams.fetch_sub(1, Ordering::AcqRel);
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Convert a [`BlockRef`] to its proto representation.
fn block_ref_to_proto(r: &BlockRef) -> BlockRefProto {
    BlockRefProto {
        round: r.round,
        author: r.author.value() as u32,
        digest: r.digest.as_ref().to_vec(),
    }
}

/// Convert an internal event to its proto representation.
fn convert_event_to_proto(event: DagVisualizerEvent) -> DagEvent {
    match event {
        DagVisualizerEvent::BlockAccepted {
            block_ref,
            timestamp_ms,
            ancestors,
            acknowledgments,
        } => DagEvent {
            event: Some(Event::BlockAccepted(BlockAcceptedEvent {
                round: block_ref.round,
                author: block_ref.author.value() as u32,
                digest: block_ref.digest.as_ref().to_vec(),
                timestamp_ms,
                ancestors: ancestors.iter().map(block_ref_to_proto).collect(),
                acknowledgments: acknowledgments.iter().map(block_ref_to_proto).collect(),
            })),
        },
        DagVisualizerEvent::LeaderCommitted(leader) => DagEvent {
            event: Some(Event::LeaderDecided(LeaderDecidedEvent {
                wave: wave_for_round(leader.round),
                leader_round: leader.round,
                leader_authority: leader.author.value() as u32,
                status: LeaderStatus::Committed.into(),
                block_digest: leader.digest.as_ref().to_vec(),
            })),
        },
        DagVisualizerEvent::LeaderSkipped(slot) => DagEvent {
            event: Some(Event::LeaderDecided(LeaderDecidedEvent {
                wave: wave_for_round(slot.round),
                leader_round: slot.round,
                leader_authority: slot.authority.value() as u32,
                status: LeaderStatus::Skipped.into(),
                block_digest: vec![],
            })),
        },
        DagVisualizerEvent::RoundAdvanced { round } => DagEvent {
            event: Some(Event::RoundAdvanced(RoundAdvancedEvent { round })),
        },
    }
}

/// Start the DAG visualizer gRPC server.
///
/// The **port** is determined by `Parameters::dag_visualizer_port`, which can
/// be set in configuration or overridden by the `DAG_VISUALIZER_PORT` env var
/// (handled in `starfish_manager.rs`).
///
/// The **bind address** (host part) defaults to `127.0.0.1`. Set the
/// `DAG_VISUALIZER_GRPC_ADDRESS` env var to override the host (e.g. `0.0.0.0`
/// for Docker inter-container access).
///
/// # Safety
/// Binding to `0.0.0.0` exposes internal DAG state on all interfaces.
/// Only use in isolated/private networks (e.g. Docker compose networks).
///
/// Returns a `(JoinHandle, shutdown_sender)` pair. Send on the oneshot to
/// trigger graceful shutdown.
pub(crate) fn start_grpc_server(
    port: u16,
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    event_tx: broadcast::Sender<DagVisualizerEvent>,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let service = DagVisualizerServiceImpl {
        context,
        dag_state,
        event_tx,
        active_streams: Arc::new(AtomicUsize::new(0)),
    };

    let host =
        std::env::var("DAG_VISUALIZER_GRPC_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], port)));

    if !addr.ip().is_loopback() {
        warn!(
            "DAG visualizer gRPC server binding to non-loopback address {addr}. \
             This exposes internal DAG state on all interfaces — \
             only use in isolated/private networks."
        );
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        info!("DAG visualizer gRPC server listening on {addr}");
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(DagVisualizerServiceServer::new(service))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
                info!("DAG visualizer gRPC server shutting down");
            })
            .await
        {
            warn!("DAG visualizer gRPC server error: {e}");
        }
    });

    (handle, shutdown_tx)
}

#[cfg(test)]
mod tests {
    use dag_visualizer_proto::dag_visualizer::dag_event::Event;
    use starfish_config::AuthorityIndex;

    use super::*;
    use crate::block_header::BlockHeaderDigest;

    fn test_block_ref(round: u32, author: u8) -> BlockRef {
        BlockRef::new(
            round,
            AuthorityIndex::new_for_test(author),
            BlockHeaderDigest::MIN,
        )
    }

    #[test]
    fn convert_block_accepted_to_proto() {
        let event = DagVisualizerEvent::BlockAccepted {
            block_ref: test_block_ref(10, 2),
            timestamp_ms: 5000,
            ancestors: Arc::from([test_block_ref(9, 1)]),
            acknowledgments: Arc::from([test_block_ref(8, 1)]),
        };

        let proto = convert_event_to_proto(event);
        match proto.event.unwrap() {
            Event::BlockAccepted(b) => {
                assert_eq!(b.round, 10);
                assert_eq!(b.author, 2);
                assert_eq!(b.timestamp_ms, 5000);
                assert_eq!(b.ancestors.len(), 1);
                assert_eq!(b.ancestors[0].round, 9);
                assert_eq!(b.ancestors[0].author, 1);
                assert_eq!(b.acknowledgments.len(), 1);
                assert_eq!(b.acknowledgments[0].round, 8);
                assert_eq!(b.acknowledgments[0].author, 1);
            }
            _ => panic!("Expected BlockAccepted"),
        }
    }

    #[test]
    fn convert_leader_committed_to_proto() {
        let event = DagVisualizerEvent::LeaderCommitted(test_block_ref(7, 1));

        let proto = convert_event_to_proto(event);
        match proto.event.unwrap() {
            Event::LeaderDecided(l) => {
                assert_eq!(l.wave, wave_for_round(7));
                assert_eq!(l.leader_round, 7);
                assert_eq!(l.leader_authority, 1);
                assert_eq!(l.status, LeaderStatus::Committed as i32);
            }
            _ => panic!("Expected LeaderDecided"),
        }
    }

    #[test]
    fn convert_leader_skipped_to_proto() {
        let slot = Slot {
            round: 7,
            authority: AuthorityIndex::new_for_test(2),
        };
        let event = DagVisualizerEvent::LeaderSkipped(slot);

        let proto = convert_event_to_proto(event);
        match proto.event.unwrap() {
            Event::LeaderDecided(l) => {
                assert_eq!(l.wave, wave_for_round(7));
                assert_eq!(l.leader_round, 7);
                assert_eq!(l.leader_authority, 2);
                assert_eq!(l.status, LeaderStatus::Skipped as i32);
                assert!(l.block_digest.is_empty());
            }
            _ => panic!("Expected LeaderDecided"),
        }
    }

    #[test]
    fn convert_round_advanced_to_proto() {
        let event = DagVisualizerEvent::RoundAdvanced { round: 42 };

        let proto = convert_event_to_proto(event);
        match proto.event.unwrap() {
            Event::RoundAdvanced(r) => {
                assert_eq!(r.round, 42);
            }
            _ => panic!("Expected RoundAdvanced"),
        }
    }
}
