// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anemo::{PeerId, Request, Response, types::response::StatusCode};
use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

use super::{Randomness, RandomnessMessage, SendSignaturesRequest};

pub(super) struct Server {
    pub(super) sender: mpsc::Sender<(RandomnessMessage, OwnedSemaphorePermit)>,
    pub(super) queued_per_peer: DashMap<PeerId, Arc<Semaphore>>,
    pub(super) max_queued_requests_per_peer: usize,
}

#[anemo::async_trait]
impl Randomness for Server {
    async fn send_signatures(
        &self,
        request: Request<SendSignaturesRequest>,
    ) -> Result<Response<()>, anemo::rpc::Status> {
        let peer_id = *request
            .peer_id()
            .ok_or_else(|| anemo::rpc::Status::internal("missing peer ID"))?;
        // The event loop drops the permit once it has handled the message.
        let permit = self
            .queued_per_peer
            .entry(peer_id)
            .or_insert_with(|| Arc::new(Semaphore::new(self.max_queued_requests_per_peer)))
            .clone()
            .try_acquire_owned()
            .map_err(|_| anemo::rpc::Status::new(StatusCode::TooManyRequests))?;
        let SendSignaturesRequest {
            epoch,
            round,
            partial_sigs,
            sig,
        } = request.into_inner();
        // Reject instead of waiting when full; the caller will retry.
        match self.sender.try_send((
            RandomnessMessage::ReceiveSignatures(peer_id, epoch, round, partial_sigs, sig),
            permit,
        )) {
            Ok(()) => Ok(anemo::Response::new(())),
            Err(mpsc::error::TrySendError::Full(_)) => {
                Err(anemo::rpc::Status::new(StatusCode::TooManyRequests))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(anemo::rpc::Status::internal("shutting down"))
            }
        }
    }
}
