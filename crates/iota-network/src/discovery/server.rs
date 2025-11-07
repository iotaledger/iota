// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, RwLock};

use anemo::{Request, Response};
use iota_config::p2p::AccessType;
use rand::seq::{IteratorRandom, SliceRandom};
use serde::{Deserialize, Serialize};

use super::{Discovery, MAX_PEERS_TO_SEND, SignedNodeInfo, State};

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

        // Create a hashmap with all known peers that are not private
        let non_private_peers: std::collections::HashMap<_, _> = state
            .known_peers
            .iter()
            .filter_map(|(peer_id, peer_info)| {
                (peer_info.access_type != AccessType::Private)
                    .then_some((*peer_id, peer_info.inner().clone()))
            })
            .collect();

        let mut known_peers = Vec::new();

        // Step 1: Add connected peers (highest priority)
        let mut connected_peers: Vec<_> = non_private_peers
            .iter()
            .filter_map(|(peer_id, peer_info)| {
                state
                    .connected_peers
                    .contains_key(peer_id)
                    .then_some(peer_info.clone())
            })
            .choose_multiple(&mut rng, MAX_PEERS_TO_SEND);
        known_peers.append(&mut connected_peers);

        // Step 2: Add not connected peers with addresses (lower priority)
        if known_peers.len() < MAX_PEERS_TO_SEND {
            let mut not_connected_with_addresses: Vec<_> = non_private_peers
                .iter()
                .filter_map(|(peer_id, peer_info)| {
                    (!state.connected_peers.contains_key(peer_id)
                        && !peer_info.addresses.is_empty())
                    .then_some(peer_info.clone())
                })
                .choose_multiple(&mut rng, MAX_PEERS_TO_SEND - known_peers.len());
            known_peers.append(&mut not_connected_with_addresses);
        }

        // Shuffle the known peers to obfuscate network topology
        known_peers.shuffle(&mut rng);

        Ok(Response::new(GetKnownPeersResponseV2 {
            own_info,
            known_peers,
        }))
    }
}
