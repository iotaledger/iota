// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anemo::{Request, Response, types::response::StatusCode};
use tokio::sync::mpsc;

use super::{Randomness, RandomnessMessage, SendSignaturesRequest};

pub(super) struct Server {
    pub(super) sender: mpsc::Sender<RandomnessMessage>,
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
        let SendSignaturesRequest {
            epoch,
            round,
            partial_sigs,
            sig,
        } = request.into_inner();
        // Reject instead of waiting when full; the caller will retry.
        match self.sender.try_send(RandomnessMessage::ReceiveSignatures(
            peer_id,
            epoch,
            round,
            partial_sigs,
            sig,
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
