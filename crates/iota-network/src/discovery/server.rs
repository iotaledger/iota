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

        let mut rng = rand::rng();

        // All known peers that are not private
        let non_private_peers = || {
            state
                .known_peers
                .iter()
                .filter(|(_, peer_info)| peer_info.access_type != AccessType::Private)
        };

        let mut known_peers = Vec::new();

        // Step 1: Add connected peers (highest priority)
        let connected_peers = non_private_peers()
            .filter_map(|(peer_id, peer_info)| {
                state
                    .connected_peers
                    .contains_key(peer_id)
                    .then_some(peer_info)
            })
            .sample(&mut rng, MAX_PEERS_TO_SEND);
        known_peers.extend(
            connected_peers
                .into_iter()
                .map(|peer_info| peer_info.inner().clone()),
        );

        // Step 2: Add not connected peers with addresses (lower priority)
        if known_peers.len() < MAX_PEERS_TO_SEND {
            let not_connected_with_addresses = non_private_peers()
                .filter_map(|(peer_id, peer_info)| {
                    (!state.connected_peers.contains_key(peer_id)
                        && !peer_info.addresses.is_empty())
                    .then_some(peer_info)
                })
                .sample(&mut rng, MAX_PEERS_TO_SEND - known_peers.len());
            known_peers.extend(
                not_connected_with_addresses
                    .into_iter()
                    .map(|peer_info| peer_info.inner().clone()),
            );
        }

        // Shuffle the known peers to obfuscate network topology
        known_peers.shuffle(&mut rng);

        Ok(Response::new(GetKnownPeersResponseV2 {
            own_info,
            known_peers,
        }))
    }
}
