// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{Arc, RwLock},
    task::{Context, Poll},
};

use anemo::{Request, Response, Result, rpc::Status, types::response::StatusCode};
use bytes::Bytes;
use dashmap::DashMap;
use futures::future::BoxFuture;
use iota_sdk_types::{CheckpointContentsDigest, CheckpointDigest};
use iota_types::{
    messages_checkpoint::{
        CertifiedCheckpointSummary as Checkpoint, CheckpointSequenceNumber, FullCheckpointContents,
        VerifiedCheckpoint,
    },
    storage::WriteStore,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

use super::{PeerHeights, PeerStateSyncInfo, StateSync, StateSyncMessage};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GetCheckpointSummaryRequest {
    Latest,
    ByDigest(CheckpointDigest),
    BySequenceNumber(CheckpointSequenceNumber),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCheckpointAvailabilityResponse {
    pub(crate) highest_synced_checkpoint: Checkpoint,
    pub(crate) lowest_available_checkpoint: CheckpointSequenceNumber,
}

/// Handshake message exchanged by both sides immediately on connect.
/// Contains everything needed to register a peer: chain identity (verified
/// genesis checkpoint), pruning watermark, and sync height.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateSyncHandshake {
    /// The verified genesis checkpoint for chain identity verification.
    pub genesis_checkpoint: Checkpoint,
    /// The highest synced checkpoint of the sending node.
    pub highest_synced_checkpoint: Checkpoint,
    /// The lowest available checkpoint (pruning watermark).
    pub lowest_available_checkpoint: CheckpointSequenceNumber,
}

pub(super) struct Server<S> {
    pub(super) store: S,
    pub(super) peer_heights: Arc<RwLock<PeerHeights>>,
    pub(super) sender: mpsc::WeakSender<StateSyncMessage>,
    /// Cached genesis checkpoint, shared with the event loop.
    pub(super) genesis_checkpoint: Arc<VerifiedCheckpoint>,
    pub(super) max_checkpoint_lookahead: u64,
}

#[anemo::async_trait]
impl<S> StateSync for Server<S>
where
    S: WriteStore + Send + Sync + 'static,
{
    /// Pushes a checkpoint summary to the server.
    /// If the checkpoint is higher than the highest verified checkpoint, it
    /// will notify the event loop to potentially sync it.
    async fn push_checkpoint_summary(
        &self,
        request: Request<Checkpoint>,
    ) -> Result<Response<()>, Status> {
        let peer_id = request
            .peer_id()
            .copied()
            .ok_or_else(|| Status::internal("unable to query sender's PeerId"))?;

        let checkpoint = request.into_inner();
        let checkpoint_seq = checkpoint.sequence_number();

        let highest_verified_checkpoint = self
            .store
            .try_get_highest_verified_checkpoint_seq_number()
            .map_err(|e| Status::internal(e.to_string()))?;

        {
            let mut peer_heights = self.peer_heights.write().unwrap();

            // Always update the peer's height so we know what they claim to have,
            // even if we don't store the checkpoint itself.
            let peer_on_same_chain = peer_heights.update_peer_height(peer_id, checkpoint_seq, None);
            if !peer_on_same_chain {
                return Ok(Response::new(()));
            }

            if checkpoint_seq
                <= highest_verified_checkpoint.saturating_add(self.max_checkpoint_lookahead)
            {
                peer_heights.insert_checkpoint(checkpoint);
            } else {
                tracing::debug!(
                    peer_id = ?peer_id,
                    checkpoint_seq = checkpoint_seq,
                    highest_verified = highest_verified_checkpoint,
                    max_lookahead = self.max_checkpoint_lookahead,
                    "not storing checkpoint summary that exceeds max lookahead"
                );
            }
        }

        // If this checkpoint is higher than our highest verified checkpoint notify the
        // event loop to potentially sync it
        if checkpoint_seq > highest_verified_checkpoint {
            if let Some(sender) = self.sender.upgrade() {
                sender.send(StateSyncMessage::StartSyncJob).await.unwrap();
            }
        }

        Ok(Response::new(()))
    }

    /// Gets a checkpoint summary by digest or sequence number, or get the
    /// latest one.
    async fn get_checkpoint_summary(
        &self,
        request: Request<GetCheckpointSummaryRequest>,
    ) -> Result<Response<Option<Checkpoint>>, Status> {
        let checkpoint = match request.inner() {
            GetCheckpointSummaryRequest::Latest => {
                self.store.try_get_highest_synced_checkpoint().map(Some)
            }
            GetCheckpointSummaryRequest::ByDigest(digest) => {
                self.store.try_get_checkpoint_by_digest(digest)
            }
            GetCheckpointSummaryRequest::BySequenceNumber(sequence_number) => self
                .store
                .try_get_checkpoint_by_sequence_number(*sequence_number),
        }
        .map_err(|e| Status::internal(e.to_string()))?
        .map(VerifiedCheckpoint::into_inner);

        Ok(Response::new(checkpoint))
    }

    /// Gets the highest synced checkpoint and the lowest available checkpoint
    /// of the node.
    async fn get_checkpoint_availability(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetCheckpointAvailabilityResponse>, Status> {
        let highest_synced_checkpoint = self
            .store
            .try_get_highest_synced_checkpoint()
            .map_err(|e| Status::internal(e.to_string()))
            .map(VerifiedCheckpoint::into_inner)?;
        let lowest_available_checkpoint = self
            .store
            .try_get_lowest_available_checkpoint()
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetCheckpointAvailabilityResponse {
            highest_synced_checkpoint,
            lowest_available_checkpoint,
        }))
    }

    /// Gets the contents of a checkpoint.
    async fn get_checkpoint_contents(
        &self,
        request: Request<CheckpointContentsDigest>,
    ) -> Result<Response<Option<FullCheckpointContents>>, Status> {
        let contents = self
            .store
            .try_get_full_checkpoint_contents(request.inner())
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(contents))
    }

    /// Handles a handshake from a newly connected peer. Registers the peer
    /// with correct chain identity, sync height, and pruning watermark in a
    /// single step — no additional queries needed.
    ///
    /// Returns our own state so the caller can register us in a single
    /// round-trip, eliminating the race where a pushed checkpoint arrives
    /// before the reverse handshake completes.
    async fn exchange_state_sync_handshake(
        &self,
        request: Request<StateSyncHandshake>,
    ) -> Result<Response<StateSyncHandshake>, Status> {
        let peer_id = request
            .peer_id()
            .copied()
            .ok_or_else(|| Status::internal("unable to query sender's PeerId"))?;

        let handshake = request.into_inner();

        let our_genesis_digest = *self.genesis_checkpoint.digest();

        let on_same_chain = *handshake.genesis_checkpoint.digest() == our_genesis_digest;

        {
            let mut guard = self.peer_heights.write().unwrap();
            guard.insert_peer_info(
                peer_id,
                PeerStateSyncInfo {
                    genesis_checkpoint_digest: *handshake.genesis_checkpoint.digest(),
                    on_same_chain_as_us: on_same_chain,
                    height: handshake.highest_synced_checkpoint.sequence_number(),
                    lowest: handshake.lowest_available_checkpoint,
                },
            );
            if on_same_chain {
                guard.insert_checkpoint(handshake.highest_synced_checkpoint.clone());
            }
        }

        if on_same_chain {
            if let Some(sender) = self.sender.upgrade() {
                // Kick the event loop so it can start syncing from the new
                // peer if they are ahead of us.  No separate push needed —
                // both sides already exchanged full checkpoint objects in
                // the handshake request/response.
                let _ = sender.send(StateSyncMessage::StartSyncJob).await;
            }
        }

        // Return our own state so the caller can register us immediately.
        let our_highest = self
            .store
            .try_get_highest_synced_checkpoint()
            .map_err(|e| Status::internal(e.to_string()))?;
        let our_lowest = self
            .store
            .try_get_lowest_available_checkpoint()
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = StateSyncHandshake {
            genesis_checkpoint: self.genesis_checkpoint.as_ref().clone().into_inner(),
            highest_synced_checkpoint: our_highest.into_inner(),
            lowest_available_checkpoint: our_lowest,
        };
        Ok(Response::new(response))
    }
}

/// [`Layer`] for limiting the size of incoming requests.
#[derive(Clone)]
pub(super) struct SizeLimitLayer {
    max_size: usize,
}

impl SizeLimitLayer {
    pub(super) fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

impl<S> tower::layer::Layer<S> for SizeLimitLayer {
    type Service = SizeLimit<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SizeLimit {
            inner,
            max_size: self.max_size,
        }
    }
}

/// Middleware for limiting the size of incoming requests.
#[derive(Clone)]
pub(super) struct SizeLimit<S> {
    inner: S,
    max_size: usize,
}

impl<S> tower::Service<Request<Bytes>> for SizeLimit<S>
where
    S: tower::Service<Request<Bytes>, Response = Response<Bytes>> + 'static + Clone + Send,
    <S as tower::Service<Request<Bytes>>>::Future: Send,
{
    type Response = Response<Bytes>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Bytes>) -> Self::Future {
        let body_size = req.body().len();
        let max_size = self.max_size;
        if body_size > max_size {
            let peer_id = req.peer_id().copied();
            tracing::info!(
                ?peer_id,
                body_size,
                max_size,
                "rejecting request that exceeds max size"
            );
            return Box::pin(async move { Ok(Response::new(Bytes::new())) });
        }
        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

/// [`Layer`] for adding a per-checkpoint limit to the number of inflight
/// GetCheckpointContent requests.
#[derive(Clone)]
pub(super) struct CheckpointContentsDownloadLimitLayer {
    inflight_per_checkpoint: Arc<DashMap<CheckpointContentsDigest, Arc<Semaphore>>>,
    max_inflight_per_checkpoint: usize,
}

impl CheckpointContentsDownloadLimitLayer {
    pub(super) fn new(max_inflight_per_checkpoint: usize) -> Self {
        Self {
            inflight_per_checkpoint: Arc::new(DashMap::new()),
            max_inflight_per_checkpoint,
        }
    }

    pub(super) fn maybe_prune_map(&self) {
        const PRUNE_THRESHOLD: usize = 5000;
        if self.inflight_per_checkpoint.len() >= PRUNE_THRESHOLD {
            self.inflight_per_checkpoint.retain(|_, semaphore| {
                semaphore.available_permits() < self.max_inflight_per_checkpoint
            });
        }
    }
}

impl<S> tower::layer::Layer<S> for CheckpointContentsDownloadLimitLayer {
    type Service = CheckpointContentsDownloadLimit<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CheckpointContentsDownloadLimit {
            inner,
            inflight_per_checkpoint: self.inflight_per_checkpoint.clone(),
            max_inflight_per_checkpoint: self.max_inflight_per_checkpoint,
        }
    }
}

/// Middleware for adding a per-checkpoint limit to the number of inflight
/// GetCheckpointContent requests.
#[derive(Clone)]
pub(super) struct CheckpointContentsDownloadLimit<S> {
    inner: S,
    inflight_per_checkpoint: Arc<DashMap<CheckpointContentsDigest, Arc<Semaphore>>>,
    max_inflight_per_checkpoint: usize,
}

impl<S> tower::Service<Request<CheckpointContentsDigest>> for CheckpointContentsDownloadLimit<S>
where
    S: tower::Service<
            Request<CheckpointContentsDigest>,
            Response = Response<Option<FullCheckpointContents>>,
            Error = Status,
        >
        + 'static
        + Clone
        + Send,
    <S as tower::Service<Request<CheckpointContentsDigest>>>::Future: Send,
    Request<CheckpointContentsDigest>: 'static + Send + Sync,
{
    type Response = Response<Option<FullCheckpointContents>>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<CheckpointContentsDigest>) -> Self::Future {
        let inflight_per_checkpoint = self.inflight_per_checkpoint.clone();
        let max_inflight_per_checkpoint = self.max_inflight_per_checkpoint;
        let mut inner = self.inner.clone();

        let fut = async move {
            let semaphore = {
                let semaphore_entry = inflight_per_checkpoint
                    .entry(*req.body())
                    .or_insert_with(|| Arc::new(Semaphore::new(max_inflight_per_checkpoint)));
                semaphore_entry.value().clone()
            };
            let permit = semaphore.try_acquire_owned().map_err(|e| match e {
                tokio::sync::TryAcquireError::Closed => {
                    anemo::rpc::Status::new(StatusCode::InternalServerError)
                }
                tokio::sync::TryAcquireError::NoPermits => {
                    anemo::rpc::Status::new(StatusCode::TooManyRequests)
                }
            })?;

            struct SemaphoreExtension(#[expect(unused)] OwnedSemaphorePermit);
            inner.call(req).await.map(move |mut response| {
                // Insert permit as extension so it's not dropped until the response is sent.
                response
                    .extensions_mut()
                    .insert(Arc::new(SemaphoreExtension(permit)));
                response
            })
        };
        Box::pin(fut)
    }
}
