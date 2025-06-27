// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, RwLock};

use anemo::{Request, Response};
use iota_config::p2p::AccessType;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};

use super::{Discovery, MAX_PEERS_TO_SEND, NodeInfo, SignedNodeInfo, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetKnownPeersResponse {
    pub own_info: NodeInfo,
    pub known_peers: Vec<NodeInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetKnownPeersResponseV2 {
    pub own_info: SignedNodeInfo,
    pub known_peers: Vec<SignedNodeInfo>,
}

pub(super) struct Server {
    pub(super) state: Arc<RwLock<State>>,
}

#[anemo::async_trait]
impl Discovery for Server {
    async fn get_known_peers(
        &self,
        request: Request<()>,
    ) -> Result<Response<GetKnownPeersResponse>, anemo::rpc::Status> {
        let resp = self.get_known_peers_v2(request).await?;
        Ok(resp.map(|body| GetKnownPeersResponse {
            own_info: body.own_info.into_data(),
            known_peers: body
                .known_peers
                .into_iter()
                .map(|e| e.into_data())
                .collect(),
        }))
    }

    async fn get_known_peers_v2(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetKnownPeersResponseV2>, anemo::rpc::Status> {
        let state = self.state.read().unwrap();
        let own_info = state
            .our_info
            .clone()
            .ok_or_else(|| anemo::rpc::Status::internal("own_info has not been initialized yet"))?;

        let mut rng = rand::thread_rng();
        // Prefer connected peers
        let mut known_peers = state
            .known_peers
            .iter()
            .filter_map(|(peer_id, peer_info)| {
                (peer_info.access_type != AccessType::Private
                    && state.connected_peers.contains_key(peer_id))
                .then_some(peer_info.inner().clone())
            })
            .choose_multiple(&mut rng, MAX_PEERS_TO_SEND);
        let mut known_not_connected_peers = state
            .known_peers
            .iter()
            .filter_map(|(peer_id, peer_info)| {
                (peer_info.access_type != AccessType::Private
                    && !state.connected_peers.contains_key(peer_id))
                .then_some(peer_info.inner().clone())
            })
            .choose_multiple(&mut rng, MAX_PEERS_TO_SEND - known_peers.len());
        known_peers.append(&mut known_not_connected_peers);

        Ok(Response::new(GetKnownPeersResponseV2 {
            own_info,
            known_peers,
        }))
    }
}
