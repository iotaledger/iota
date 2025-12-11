// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::{Arc, RwLock},
    time::Instant,
};

use anemo::{
    Network, Peer, PeerId, Request, Response,
    types::{PeerEvent, PeerInfo},
};
use fastcrypto::ed25519::{Ed25519PublicKey, Ed25519Signature};
use futures::StreamExt;
use iota_config::p2p::{AccessType, DiscoveryConfig, P2pConfig, SeedPeer};
use iota_sdk_types::crypto::IntentScope;
use iota_types::{
    crypto::{NetworkKeyPair, Signer, ToFromBytes, VerifyingKey},
    digests::Digest,
    message_envelope::{Envelope, Message, VerifiedEnvelope},
    multiaddr::Multiaddr,
};
use serde::{Deserialize, Serialize};
use tap::{Pipe, TapFallible};
use tokio::{
    sync::{broadcast::error::RecvError, oneshot, watch},
    task::{AbortHandle, JoinSet},
};
use tracing::{debug, info, trace};

const ONE_DAY_MILLISECONDS: u64 = 24 * 60 * 60 * 1_000;
const MAX_ADDRESS_LENGTH: usize = 300;
const MAX_PEERS_TO_SEND: usize = 200;
const MAX_ADDRESSES_PER_PEER: usize = 2;

// Includes the generated Discovery code from the OUT_DIR
mod generated {
    include!(concat!(env!("OUT_DIR"), "/iota.Discovery.rs"));
}
mod builder;
mod metrics;
mod server;
#[cfg(test)]
mod tests;

pub use builder::{Builder, Handle, UnstartedDiscovery};
pub use generated::{
    discovery_client::DiscoveryClient,
    discovery_server::{Discovery, DiscoveryServer},
};
pub use server::GetKnownPeersResponseV2;

use self::metrics::Metrics;

/// The internal discovery state shared between the main event loop and the
/// request handler
struct State {
    our_info: Option<SignedNodeInfo>,
    connected_peers: HashMap<PeerId, ()>,
    known_peers: HashMap<PeerId, VerifiedSignedNodeInfo>,
    /// Cooldown list for peers whose address verification failed recently
    /// Maps PeerId to the instant when the verification failed.
    /// Can't be spoofed because the peer_id is part of the signed info.
    address_verification_cooldown: HashMap<PeerId, Instant>,
}

/// The information necessary to dial another peer.
///
/// `NodeInfo` contains all the information that is shared with other nodes via
/// the discovery service to advertise how a node can be reached.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeInfo {
    pub peer_id: PeerId,
    pub addresses: Vec<Multiaddr>,

    /// Creation time.
    ///
    /// This is used to determine which of two NodeInfo's from the same PeerId
    /// should be retained.
    pub timestamp_ms: u64,

    pub access_type: AccessType,
}

impl NodeInfo {
    fn sign(self, keypair: &NetworkKeyPair) -> SignedNodeInfo {
        let msg = bcs::to_bytes(&self).expect("BCS serialization should not fail");
        let sig = keypair.sign(&msg);
        SignedNodeInfo::new_from_data_and_sig(self, sig)
    }
}

pub type SignedNodeInfo = Envelope<NodeInfo, Ed25519Signature>;

pub type VerifiedSignedNodeInfo = VerifiedEnvelope<NodeInfo, Ed25519Signature>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NodeInfoDigest(Digest);

impl NodeInfoDigest {
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(Digest::new(digest))
    }
}

impl Message for NodeInfo {
    type DigestType = NodeInfoDigest;
    const SCOPE: IntentScope = IntentScope::DiscoveryPeers;

    fn digest(&self) -> Self::DigestType {
        unreachable!("NodeInfoDigest is not used today")
    }
}

#[derive(Clone, Debug, Default)]
/// Contains a new list of available trusted peers.
pub struct TrustedPeerChangeEvent {
    pub new_committee: Vec<PeerInfo>,
    pub old_committee: Vec<PeerInfo>,
}

struct DiscoveryEventLoop {
    config: P2pConfig,
    discovery_config: Arc<DiscoveryConfig>,
    allowlisted_peers: Arc<HashMap<PeerId, Option<Multiaddr>>>,
    network: Network,
    keypair: NetworkKeyPair,
    tasks: JoinSet<()>,
    pending_dials: HashMap<PeerId, AbortHandle>,
    dial_seed_peers_task: Option<AbortHandle>,
    shutdown_handle: oneshot::Receiver<()>,
    state: Arc<RwLock<State>>,
    trusted_peer_change_rx: watch::Receiver<TrustedPeerChangeEvent>,
    metrics: Metrics,
}

impl DiscoveryEventLoop {
    /// Starts the discovery event loop.
    pub async fn start(mut self) {
        info!("Discovery started");

        self.construct_our_info();
        self.configure_preferred_peers();

        let mut interval = tokio::time::interval(self.discovery_config.interval_period());
        let mut cleanup_interval =
            tokio::time::interval(self.discovery_config.cooldown_cleanup_interval());
        let mut peer_events = {
            let (subscriber, _peers) = self.network.subscribe().unwrap();
            subscriber
        };

        loop {
            tokio::select! {
                now = interval.tick() => {
                    let now_unix = now_unix();
                    self.handle_tick(now.into_std(), now_unix);
                }
                now = cleanup_interval.tick() => {
                    cleanup_verification_failure_cooldown(&self.state, now.into_std(), &self.discovery_config);
                }
                peer_event = peer_events.recv() => {
                    self.handle_peer_event(peer_event);
                },
                // This is signaled when new trusted peer (committee member) is added.
                Ok(()) = self.trusted_peer_change_rx.changed() => {
                    let event: TrustedPeerChangeEvent = self.trusted_peer_change_rx.borrow_and_update().clone();
                    self.handle_trusted_peer_change_event(event);
                }
                // Handles the result of a task from tasks.
                Some(task_result) = self.tasks.join_next() => {
                    match task_result {
                        Ok(()) => {},
                        Err(e) => {
                            if e.is_cancelled() {
                                // avoid crashing on ungraceful shutdown.
                            } else if e.is_panic() {
                                // propagate panics.
                                std::panic::resume_unwind(e.into_panic());
                            } else {
                                panic!("task failed: {e}");
                            }
                        },
                    };
                },
                // Once the shutdown notification is resolved we can terminate the event loop.
                _ = &mut self.shutdown_handle => {
                    break;
                }
            }
        }

        info!("Discovery ended");
    }

    /// Constructs [`NodeInfo`] of the node.
    fn construct_our_info(&mut self) {
        if self.state.read().unwrap().our_info.is_some() {
            return;
        }

        let address = self
            .config
            .external_address
            .clone()
            .and_then(|addr| {
                // Validate that our external address is suitable for public announcement
                if addr.is_valid_public_anemo_address(self.discovery_config.allow_private_addresses()) {
                    addr.to_anemo_address().ok().map(|_| addr)
                } else {
                    info!(
                        "External address {} is not suitable for public announcement (invalid/private/unroutable)",
                        addr
                    );
                    None
                }
            })
            .into_iter()
            .collect();

        let our_info = NodeInfo {
            peer_id: self.network.peer_id(),
            addresses: address,
            timestamp_ms: now_unix(),
            access_type: self.discovery_config.access_type(),
        }
        .sign(&self.keypair);

        self.state.write().unwrap().our_info = Some(our_info);
    }

    /// Configures known peers list in [`Network`] using allowlisted peers and
    /// seed peers.
    fn configure_preferred_peers(&mut self) {
        // Iterates over the allowlisted peers and seed peers to check if they have
        // an address that can be converted to anemo address. If they do, they are added
        // to the known peers list.
        for (peer_id, address) in self
            .discovery_config
            .allowlisted_peers
            .iter()
            .map(|ap| (ap.peer_id, ap.address.clone()))
            .chain(self.config.seed_peers.iter().filter_map(|sp| {
                sp.peer_id
                    .map(|peer_id| (peer_id, Some(sp.address.clone())))
            }))
        {
            let anemo_address = if let Some(address) = address {
                let Ok(address) = address.to_anemo_address() else {
                    debug!(p2p_address=?address, "Can't convert p2p address to anemo address");
                    continue;
                };
                Some(address)
            } else {
                None
            };

            // TODO: once we have `PeerAffinity::Allowlisted` we should update allowlisted
            // peers' affinity.
            let peer_info = anemo::types::PeerInfo {
                peer_id,
                affinity: anemo::types::PeerAffinity::High,
                address: anemo_address.into_iter().collect(),
            };
            debug!(?peer_info, "Add configured preferred peer");
            self.network.known_peers().insert(peer_info);
        }
    }

    fn update_our_info_timestamp(&mut self, now_unix: u64) {
        let state = &mut self.state.write().unwrap();
        if let Some(our_info) = &state.our_info {
            let mut data = our_info.data().clone();
            data.timestamp_ms = now_unix;
            state.our_info = Some(data.sign(&self.keypair));
        }
    }

    /// Handles a [`TrustedPeerChangeEvent`] by updating the known peers with
    /// the latest trusted new peers without deleting the allowlisted peers.
    fn handle_trusted_peer_change_event(
        &mut self,
        trusted_peer_change_event: TrustedPeerChangeEvent,
    ) {
        let TrustedPeerChangeEvent {
            new_committee,
            old_committee,
        } = trusted_peer_change_event;

        let new_peer_ids = new_committee
            .iter()
            .map(|peer| peer.peer_id)
            .collect::<HashSet<_>>();

        // Remove peers from old_committee who are not in new_committee and are not in
        // self.allowlisted_peers.
        let to_remove = old_committee
            .iter()
            .map(|peer_info| &peer_info.peer_id)
            .filter(|old_peer_id| {
                !new_peer_ids.contains(old_peer_id)
                    && !self.allowlisted_peers.contains_key(old_peer_id)
            });

        // Add the new_committee to the known peers skipping self peer.
        // This will update the PeerInfo for those who are already in the
        // committee and have updated their PeerInfo.
        let to_insert = new_committee
            .into_iter()
            .filter(|peer_info| !self.network.peer_id().eq(&peer_info.peer_id));

        let (removed, updated_or_inserted) = self
            .network
            .known_peers()
            .batch_update(to_remove, to_insert.clone());

        // Actually removed, may differ from `to_remove`
        let removed: Vec<_> = removed
            .into_iter()
            .filter_map(|removed| removed.map(|info| info.peer_id))
            .collect();
        let mut updated = Vec::new();
        let mut inserted = Vec::new();
        for (replaced_val, to_insert_val) in updated_or_inserted.into_iter().zip(to_insert) {
            if replaced_val.is_some() {
                updated.push(to_insert_val.peer_id);
            } else {
                inserted.push(to_insert_val.peer_id);
            }
        }
        debug!(
            "Trusted peer change event: removed {removed:?}, updated {updated:?}, inserted {inserted:?}",
        );
    }

    /// Handles a [`PeerEvent`].
    ///
    /// * NewPeer: Adds the peer to the connected peers list and queries the
    ///   peer for their known peers.
    /// * LostPeer: Removes the peer from the connected peers list.
    /// * Closed: Panics if the channel is closed.
    fn handle_peer_event(&mut self, peer_event: Result<PeerEvent, RecvError>) {
        match peer_event {
            Ok(PeerEvent::NewPeer(peer_id)) => {
                if let Some(peer) = self.network.peer(peer_id) {
                    // Adds the peer to the connected peers list.
                    self.state
                        .write()
                        .unwrap()
                        .connected_peers
                        .insert(peer_id, ());

                    // Queries the new node for any peers.
                    self.tasks.spawn(query_peer_for_their_known_peers(
                        peer,
                        self.network.clone(),
                        self.state.clone(),
                        self.metrics.clone(),
                        self.allowlisted_peers.clone(),
                        self.discovery_config.clone(),
                    ));
                }
            }
            Ok(PeerEvent::LostPeer(peer_id, _)) => {
                self.state.write().unwrap().connected_peers.remove(&peer_id);
            }

            Err(RecvError::Closed) => {
                panic!("PeerEvent channel shouldn't be able to be closed");
            }

            Err(RecvError::Lagged(_)) => {
                trace!("State-Sync fell behind processing PeerEvents");
            }
        }
    }

    /// This function performs several tasks:
    ///
    /// 1. Update the timestamp of our own info.
    /// 2. Queries a subset of connected peers for their known peers.
    /// 3. Culls old known peers older than a day.
    /// 4. Cleans out the pending_dials, dial_seed_peers_task if it's done.
    /// 5. Selects a subset of known peers to dial if we're not connected to
    ///    enough peers.
    /// 6. If we have no neighbors and we aren't presently trying to connect to
    ///    anyone we need to try the seed peers.
    fn handle_tick(&mut self, _now: std::time::Instant, now_unix: u64) {
        self.update_our_info_timestamp(now_unix);

        self.tasks
            .spawn(query_connected_peers_for_their_known_peers(
                self.network.clone(),
                self.discovery_config.clone(),
                self.state.clone(),
                self.metrics.clone(),
                self.allowlisted_peers.clone(),
            ));

        // Culls old known peers older than a day.
        self.state
            .write()
            .unwrap()
            .known_peers
            .retain(|_k, v| now_unix.saturating_sub(v.timestamp_ms) < ONE_DAY_MILLISECONDS);

        // Cleans out the pending_dials.
        self.pending_dials.retain(|_k, v| !v.is_finished());
        // Cleans out the dial_seed_peers_task if it's done.
        if let Some(abort_handle) = &self.dial_seed_peers_task {
            if abort_handle.is_finished() {
                self.dial_seed_peers_task = None;
            }
        }

        // Selects a subset of known peers to dial if we're not connected to enough
        // peers.
        let state = self.state.read().unwrap();
        let eligible: Vec<_> = state
            .known_peers
            .iter()
            .filter(|(&peer_id, info)| {
                peer_id != self.network.peer_id()
                    && !info.addresses.is_empty() // Peer has addresses we can dial
                    && !state.connected_peers.contains_key(&peer_id) // We're not already connected
                    && !self.pending_dials.contains_key(&peer_id) // There is no pending dial to this node
            })
            .map(|(&peer_id, info)| (peer_id, info.clone()))
            .collect();

        // No need to connect to any more peers if we're already connected to a bunch
        let number_of_connections = state.connected_peers.len();
        let number_to_dial = std::cmp::min(
            eligible.len(),
            self.discovery_config
                .target_concurrent_connections()
                .saturating_sub(number_of_connections),
        );

        // Randomly selects the number_to_dial of peers to connect to.
        for (peer_id, info) in rand::seq::SliceRandom::choose_multiple(
            eligible.as_slice(),
            &mut rand::thread_rng(),
            number_to_dial,
        ) {
            let abort_handle = self.tasks.spawn(try_to_connect_to_peer(
                self.network.clone(),
                info.data().to_owned(),
            ));
            self.pending_dials.insert(*peer_id, abort_handle);
        }

        // If we aren't connected to anything and we aren't presently trying to connect
        // to anyone we need to try the seed peers
        if self.dial_seed_peers_task.is_none()
            && state.connected_peers.is_empty()
            && self.pending_dials.is_empty()
            && !self.config.seed_peers.is_empty()
        {
            let abort_handle = self.tasks.spawn(try_to_connect_to_seed_peers(
                self.network.clone(),
                self.discovery_config.clone(),
                self.config.seed_peers.clone(),
            ));

            self.dial_seed_peers_task = Some(abort_handle);
        }
    }
}

/// Verifies that a peer actually controls the addresses they claim by
/// attempting to establish a connection. This prevents address spoofing
/// attacks. Tries all addresses individually and returns the list of
/// addresses that were successfully verified.
/// This should only be used for not-yet-connected peers.
async fn verify_address_ownership(
    network: &Network,
    peer_info: &SignedNodeInfo,
    config: &DiscoveryConfig,
) -> Vec<Multiaddr> {
    // Try each address individually and collect the ones that work
    let verification_futures: Vec<_> = peer_info
        .addresses
        .iter()
        .map(|address| {
            let network = network.clone();
            let peer_id = peer_info.peer_id;
            let address = address.clone();
            Box::pin(async move {
                if let Ok(anemo_address) = address.to_anemo_address() {
                    // Check again if we're connected before attempting verification
                    // to handle race conditions where connection happens during verification
                    if network.peer(peer_id).is_some() {
                        debug!(
                            "Peer {} connected during verification, trusting address {}",
                            peer_id, address
                        );
                        return Some(address);
                    }

                    // Try to connect to the claimed address with the claimed peer_id
                    match tokio::time::timeout(
                        config.address_verification_timeout(),
                        network.connect_with_peer_id(anemo_address, peer_id),
                    )
                    .await
                    {
                        Ok(Ok(_connection)) => {
                            debug!(
                                "Address verification succeeded for peer {} at address {}",
                                peer_id, address
                            );

                            // Disconnect immediately since this was just for verification
                            let _ = network.disconnect(peer_id);
                            return Some(address);
                        }
                        Ok(Err(e)) => {
                            debug!(
                                "Address verification failed for peer {} at address {}: {}",
                                peer_id, address, e
                            );
                        }
                        Err(_timeout) => {
                            debug!(
                                "Address verification timed out for peer {} at address {}",
                                peer_id, address
                            );
                        }
                    }
                }
                None
            })
        })
        .collect();

    // Wait for all verification attempts to complete and collect successful ones
    let results = futures::future::join_all(verification_futures).await;
    results.into_iter().flatten().collect()
}

async fn try_to_connect_to_peer(network: Network, info: NodeInfo) {
    debug!("Connecting to peer {info:?}");
    for multiaddr in &info.addresses {
        if let Ok(address) = multiaddr.to_anemo_address() {
            // Ignore the result and just log the error if there is one
            if network
                .connect_with_peer_id(address, info.peer_id)
                .await
                .tap_err(|e| {
                    debug!(
                        "error dialing {} at address '{}': {e}",
                        info.peer_id, multiaddr
                    )
                })
                .is_ok()
            {
                return;
            }
        }
    }
}

async fn try_to_connect_to_seed_peers(
    network: Network,
    config: Arc<DiscoveryConfig>,
    seed_peers: Vec<SeedPeer>,
) {
    debug!(?seed_peers, "Connecting to seed peers");
    let network = &network;

    futures::stream::iter(seed_peers.into_iter().filter_map(|seed| {
        seed.address
            .to_anemo_address()
            .ok()
            .map(|address| (seed, address))
    }))
    .for_each_concurrent(
        config.target_concurrent_connections(),
        |(seed, address)| async move {
            // Ignores the result and just logs the error if there is one.
            let _ = if let Some(peer_id) = seed.peer_id {
                network.connect_with_peer_id(address, peer_id).await
            } else {
                network.connect(address).await
            }
            .tap_err(|e| debug!("error dialing multiaddr '{}': {e}", seed.address));
        },
    )
    .await;
}

/// Wrapper for SignedNodeInfo that implements Hash based on all fields (data +
/// signature)
#[derive(Clone, Debug)]
struct HashableSignedNodeInfo(SignedNodeInfo);

impl std::hash::Hash for HashableSignedNodeInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.peer_id.hash(state);
        self.0.addresses.hash(state);
        self.0.timestamp_ms.hash(state);
        state.write_isize(self.0.access_type as isize);
        self.0.auth_sig().as_bytes().hash(state);
    }
}

impl PartialEq for HashableSignedNodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.0.peer_id == other.0.peer_id
            && self.0.addresses == other.0.addresses
            && self.0.timestamp_ms == other.0.timestamp_ms
            && self.0.access_type == other.0.access_type
            && self.0.auth_sig() == other.0.auth_sig()
    }
}

impl Eq for HashableSignedNodeInfo {}

/// Deduplicates peer infos based on their complete content (data + signature)
fn deduplicate_peers(peers: Vec<SignedNodeInfo>) -> Vec<SignedNodeInfo> {
    peers
        .into_iter()
        .map(HashableSignedNodeInfo)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|wrapped| wrapped.0)
        .collect()
}

async fn query_peer_for_their_known_peers(
    peer: Peer,
    network: Network,
    state: Arc<RwLock<State>>,
    metrics: Metrics,
    allowlisted_peers: Arc<HashMap<PeerId, Option<Multiaddr>>>,
    config: Arc<DiscoveryConfig>,
) {
    let mut client = DiscoveryClient::new(peer);

    let request = Request::new(()).with_timeout(config.peer_query_timeout());
    let found_peers = client
        .get_known_peers_v2(request)
        .await
        .ok()
        .map(Response::into_inner)
        .map(
            |GetKnownPeersResponseV2 {
                 own_info,
                 mut known_peers,
             }| {
                // Limit each client to MAX_PEERS_TO_SEND
                known_peers.truncate(MAX_PEERS_TO_SEND);
                if !own_info.addresses.is_empty() {
                    known_peers.push(own_info)
                }
                known_peers
            },
        );

    if let Some(found_peers) = found_peers {
        update_known_peers(
            &network,
            state,
            metrics,
            deduplicate_peers(found_peers),
            allowlisted_peers,
            &config,
        )
        .await;
    }
}

/// Queries a subset of neighbors for their known peers.
async fn query_connected_peers_for_their_known_peers(
    network: Network,
    config: Arc<DiscoveryConfig>,
    state: Arc<RwLock<State>>,
    metrics: Metrics,
    allowlisted_peers: Arc<HashMap<PeerId, Option<Multiaddr>>>,
) {
    use rand::seq::IteratorRandom;

    // Randomly selects a subset of neighbors to query.
    let peers_to_query = network
        .peers()
        .into_iter()
        .flat_map(|id| network.peer(id))
        .choose_multiple(&mut rand::thread_rng(), config.peers_to_query());

    let peer_query_timeout = config.peer_query_timeout();

    // Queries the selected neighbors for their known peers in parallel.
    let found_peers = peers_to_query
        .into_iter()
        .map(DiscoveryClient::new)
        .map(|mut client| async move {
            let request = Request::new(()).with_timeout(peer_query_timeout);
            client
                .get_known_peers_v2(request)
                .await
                .ok()
                .map(Response::into_inner)
                .map(
                    |GetKnownPeersResponseV2 {
                         own_info,
                         mut known_peers,
                     }| {
                        // Limit each client to MAX_PEERS_TO_SEND
                        known_peers.truncate(MAX_PEERS_TO_SEND);
                        if !own_info.addresses.is_empty() {
                            known_peers.push(own_info)
                        }
                        known_peers
                    },
                )
        })
        .pipe(futures::stream::iter)
        .buffer_unordered(config.peers_to_query())
        .filter_map(std::future::ready)
        .flat_map(futures::stream::iter)
        .collect::<Vec<_>>()
        .await;

    update_known_peers(
        &network,
        state,
        metrics,
        deduplicate_peers(found_peers),
        allowlisted_peers,
        &config,
    )
    .await;
}

/// Cleans up old entries from the verification failure cooldown list
fn cleanup_verification_failure_cooldown(
    state: &Arc<RwLock<State>>,
    now_instant: Instant,
    config: &DiscoveryConfig,
) {
    // Skip cleanup if cooldown is disabled
    if !config.is_address_verification_cooldown_enabled() {
        return;
    }

    // First, check with a read lock to see if there are any entries to remove
    let peers_to_remove: Vec<PeerId> = {
        let state_guard = state.read().unwrap();
        let cooldown_duration = config.address_verification_failure_cooldown();

        state_guard
            .address_verification_cooldown
            .iter()
            .filter_map(|(&peer_id, &failure_instant)| {
                if now_instant.duration_since(failure_instant) >= cooldown_duration {
                    Some(peer_id)
                } else {
                    None
                }
            })
            .collect()
    };

    // Only acquire write lock if we actually have entries to remove
    if !peers_to_remove.is_empty() {
        let mut state_guard = state.write().unwrap();

        // Remove the expired entries
        for peer_id in &peers_to_remove {
            state_guard.address_verification_cooldown.remove(peer_id);
            debug!(
                "Removing peer {} from verification failure cooldown",
                peer_id
            );
        }

        debug!(
            "Cleaned up {} entries from verification failure cooldown",
            peers_to_remove.len()
        );
    }
}

fn verify_peer_infos(
    state: &Arc<RwLock<State>>,
    found_peers: Vec<Envelope<NodeInfo, Ed25519Signature>>,
    allowlisted_peers: Arc<HashMap<PeerId, Option<Multiaddr>>>,
    now_instant: Instant,
    config: &DiscoveryConfig,
) -> Vec<VerifiedSignedNodeInfo> {
    let now_unix = now_unix();

    // Acquire read lock once to get our peer ID and filter peers by cooldown
    let (our_peer_id, found_peers) = {
        let state_guard = state.read().unwrap();
        let our_peer_id = state_guard.our_info.clone().unwrap().peer_id;

        // Filter out peers that are in cooldown while holding the lock
        let filtered_peers: Vec<_> = found_peers
            .into_iter()
            .filter(|peer_info| {
                // Skip cooldown check if it's disabled
                if !config.is_address_verification_cooldown_enabled() {
                    return true;
                }

                // Check if this peer is in the address verification cooldown.
                // Even if the signature is not checked yet, it would be pointless to verify if
                // that peer_id is marked for cooldown.
                match state_guard.address_verification_cooldown.get(&peer_info.peer_id) {
                    Some(&failure_instant) => {
                        let time_since_failure = now_instant.duration_since(failure_instant);
                        let cooldown_duration = config.address_verification_failure_cooldown();
                        if time_since_failure >= cooldown_duration {
                            true // Cooldown expired
                        } else {
                            debug!(
                                "Peer {} ({}) is in verification failure cooldown (failed {:.1}s ago, cooldown: {:.1}s)",
                                peer_info.peer_id,
                                peer_info.addresses.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "),
                                time_since_failure.as_secs_f32(),
                                cooldown_duration.as_secs_f32()
                            );
                            false // Still in cooldown
                        }
                    }
                    None => true, // Not in cooldown
                }
            })
            .collect();

        (our_peer_id, filtered_peers)
    };

    let mut latest_verified_peer_infos: HashMap<PeerId, VerifiedSignedNodeInfo> =
        HashMap::with_capacity(found_peers.len());

    for peer_info in found_peers.into_iter() {
        // Skip peers whose timestamp is too far in the future from our clock
        // or that are too old
        if peer_info.timestamp_ms > now_unix.saturating_add(30 * 1_000) // 30 seconds
            || now_unix.saturating_sub(peer_info.timestamp_ms) > ONE_DAY_MILLISECONDS
        {
            continue;
        }

        // Skip our own info
        if peer_info.peer_id == our_peer_id {
            continue;
        }

        // If Peer is Private, and not in our allowlist, skip it.
        if peer_info.access_type == AccessType::Private
            && !allowlisted_peers.contains_key(&peer_info.peer_id)
        {
            continue;
        }

        // Skip entries that have no or too many addresses as a means to cap the size of
        // a node's info. This also means we don't update entries when peers remove all
        // their addresses. Those entries will eventually be removed after
        // ONE_DAY_MILLISECONDS.
        if peer_info.addresses.is_empty() || peer_info.addresses.len() > MAX_ADDRESSES_PER_PEER {
            continue;
        }

        // Verify that all addresses provided are valid anemo addresses and not
        // private/unroutable
        if !peer_info.addresses.iter().all(|addr| {
            addr.len() < MAX_ADDRESS_LENGTH
                && addr.is_valid_public_anemo_address(config.allow_private_addresses())
        }) {
            debug!(
                "Rejecting peer {} due to invalid, private, or unroutable addresses: {:?}",
                peer_info.peer_id, peer_info.addresses
            );
            continue;
        }

        // Verify the signature
        let Ok(public_key) = Ed25519PublicKey::from_bytes(&peer_info.peer_id.0) else {
            debug!(
                // This should never happen.
                "Failed to convert anemo PeerId {:?} to Ed25519PublicKey",
                peer_info.peer_id
            );
            continue;
        };
        let msg = bcs::to_bytes(peer_info.data()).expect("BCS serialization should not fail");
        if let Err(e) = public_key.verify(&msg, peer_info.auth_sig()) {
            debug!(
                "Discovery failed to verify signature for NodeInfo for peer {:?}: {e:?}",
                peer_info.peer_id
            );
            // TODO: consider denylisting the source of bad NodeInfo from future requests.
            continue;
        }
        let verified_peer_info = VerifiedSignedNodeInfo::new_from_verified(peer_info);

        // Keep only the latest entry for each peer_id based on timestamp
        latest_verified_peer_infos
            .entry(verified_peer_info.peer_id)
            .and_modify(|existing| {
                if verified_peer_info.timestamp_ms > existing.timestamp_ms {
                    *existing = verified_peer_info.clone();
                }
            })
            .or_insert(verified_peer_info);
    }

    latest_verified_peer_infos.into_values().collect()
}

/// Verifies the addresses of the given peers by attempting to connect to them.
/// Returns a tuple containing:
/// - A list of peers along with their successfully verified addresses
/// - A list of peer IDs that failed verification (to be added to cooldown)
///
/// Times out after the configured total timeout to prevent DOS attacks.
async fn verify_addresses_of_peers(
    network: &Network,
    peers: Vec<VerifiedEnvelope<NodeInfo, Ed25519Signature>>,
    config: &DiscoveryConfig,
) -> (
    Vec<(VerifiedEnvelope<NodeInfo, Ed25519Signature>, Vec<Multiaddr>)>,
    Vec<NodeInfo>,
) {
    let peers_count = peers.len();
    let verification_stream = futures::stream::iter(peers.into_iter().map(|verified_peer_info| {
        let network = network.clone();
        async move {
            let valid_addresses =
                verify_address_ownership(&network, &verified_peer_info, config).await;
            (verified_peer_info, valid_addresses)
        }
    }))
    .buffer_unordered(config.max_concurrent_address_verifications()); // Limit concurrent verifications to avoid overwhelming the network

    let mut address_verification_results = Vec::with_capacity(peers_count);
    let mut verification_stream = std::pin::Pin::new(Box::new(verification_stream));

    match tokio::time::timeout(config.address_verification_total_timeout(), async {
        while let Some(result) = verification_stream.next().await {
            address_verification_results.push(result);
        }
    })
    .await
    {
        Ok(_) => {
            debug!(
                "Address verification completed successfully for {} peers",
                address_verification_results.len()
            );
        }
        Err(_) => {
            debug!(
                "Address verification timed out after {}s, but collected {} partial results out of {} peers",
                config.address_verification_total_timeout().as_secs(),
                address_verification_results.len(),
                peers_count
            );
        }
    };

    // Collect all verified peers with their verified addresses and peers that
    // failed verification
    let mut verified_peers = Vec::new();
    let mut failed_peers = Vec::new();

    for (verified_peer_info, verified_addresses) in address_verification_results {
        if verified_addresses.is_empty() {
            debug!(
                "Rejecting peer {} ({}) due to failed address verification for all addresses",
                verified_peer_info.peer_id,
                verified_peer_info
                    .addresses
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            failed_peers.push(verified_peer_info.data().to_owned());
        } else {
            verified_peers.push((verified_peer_info, verified_addresses));
        }
    }

    (verified_peers, failed_peers)
}

/// Updates the known peers list with the found peers. The found peer is ignored
/// if it is too old or too far in the future from our clock.
/// If a peer is already known, the NodeInfo is updated, otherwise the peer is
/// inserted if at least one address is verified.
/// New or changed addresses are verified to prevent spoofing attacks.
async fn update_known_peers(
    network: &Network,
    state: Arc<RwLock<State>>,
    metrics: Metrics,
    found_peers: Vec<SignedNodeInfo>,
    allowlisted_peers: Arc<HashMap<PeerId, Option<Multiaddr>>>,
    config: &DiscoveryConfig,
) {
    let now_instant = Instant::now();

    // Verify all peer infos and filter out invalid ones, filter by latest timestamp
    // per peer_id. This does not verify address ownership yet.
    let verified_peer_infos = verify_peer_infos(
        &state,
        found_peers,
        allowlisted_peers.clone(),
        now_instant,
        config,
    );

    if verified_peer_infos.is_empty() {
        // No verified peers to process, we're done
        return;
    }

    // We need to loop over the verified peers and compare them to the existing
    // known peers. If the timestamp is newer and the addresses have changed, we
    // need to verify the new addresses and handle potential address conflicts
    // with existing entries afterwards. If the timestamp is newer but the
    // addresses are the same, we can just update the timestamp without
    // verification.
    let mut peers_to_update_directly = Vec::new();
    let mut peers_with_addresses_to_verify = Vec::new();
    {
        let state_guard = state.read().unwrap();
        for verified_peer_info in verified_peer_infos {
            // Skip address verification for allowlisted peers (they are trusted)
            let is_allowlisted = allowlisted_peers.contains_key(&verified_peer_info.peer_id);
            let is_connected = state_guard
                .connected_peers
                .contains_key(&verified_peer_info.peer_id);

            match state_guard.known_peers.get(&verified_peer_info.peer_id) {
                Some(existing_peer) => {
                    // Existing peer, check if the timestamp is newer, otherwise ignore
                    if verified_peer_info.timestamp_ms > existing_peer.timestamp_ms {
                        // Check if the addresses have changed
                        if verified_peer_info.addresses != existing_peer.addresses {
                            if is_allowlisted || is_connected {
                                // Allowlisted or connected peers are trusted, skip verification
                                peers_to_update_directly.push(verified_peer_info);
                            } else {
                                // Addresses have changed, we need to verify.
                                peers_with_addresses_to_verify.push(verified_peer_info);
                            }
                        } else {
                            // Only timestamp changed, no need to verify addresses
                            peers_to_update_directly.push(verified_peer_info);
                        }
                    }
                }
                None => {
                    if is_allowlisted {
                        // Allowlisted can be added without verification
                        peers_to_update_directly.push(verified_peer_info);
                    } else {
                        // Unknown peer, always verify
                        peers_with_addresses_to_verify.push(verified_peer_info);
                    }
                }
            }
        }
    }

    if peers_to_update_directly.is_empty() && peers_with_addresses_to_verify.is_empty() {
        // No peers to update or verify, we're done
        return;
    }

    // Verify addresses for peers that need verification
    let (mut verified_peers, failed_peer_infos) = match peers_with_addresses_to_verify.is_empty() {
        false => verify_addresses_of_peers(network, peers_with_addresses_to_verify, config).await,
        true => Default::default(),
    };

    // If we have neither verified peers nor peers to update directly, nor failed
    // peers, we're done
    if verified_peers.is_empty()
        && peers_to_update_directly.is_empty()
        && failed_peer_infos.is_empty()
    {
        return;
    }

    {
        // First check if multiple verified peers claim the same address
        let mut address_to_peer: HashMap<Multiaddr, VerifiedSignedNodeInfo> =
            HashMap::with_capacity(verified_peers.len() * MAX_ADDRESSES_PER_PEER);
        let mut new_peers_to_reject = HashSet::new();
        for (new_peer_info, verified_addresses) in &verified_peers {
            for address in verified_addresses {
                match address_to_peer.entry(address.clone()) {
                    // Replaces the peer if the new one is newer.
                    Entry::Occupied(mut existing_entry) => {
                        if new_peer_info.timestamp_ms > existing_entry.get().timestamp_ms {
                            new_peers_to_reject.insert(existing_entry.get().peer_id);
                            existing_entry.insert(new_peer_info.clone());
                        } else {
                            new_peers_to_reject.insert(new_peer_info.peer_id);
                        }
                    }
                    // Inserts the peer if it doesn't exist.
                    Entry::Vacant(v) => {
                        v.insert(new_peer_info.clone());
                    }
                }
            }
        }

        // Update the known peers state with all changes in a single write lock
        // acquisition.
        let mut state_guard = state.write().unwrap();

        // Add all failed peers to verification failure cooldown (if enabled)
        if config.is_address_verification_cooldown_enabled() {
            for peer_info in failed_peer_infos {
                state_guard
                    .address_verification_cooldown
                    .insert(peer_info.peer_id, now_instant);
                debug!(
                    "Added peer {} ({}) to verification failure cooldown for {:.1}s",
                    peer_info.peer_id,
                    peer_info
                        .addresses
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    config.address_verification_failure_cooldown().as_secs_f32()
                );
            }
        }

        // First insert/update the peers that didn't need verification
        for peer in peers_to_update_directly {
            insert_or_update_peer(&mut state_guard, peer, &metrics);
        }

        if !address_to_peer.is_empty() {
            // Remove existing peers that have conflicting addresses with the new peers, if
            // their timestamp is older than the new peer's timestamp. (Skip
            // connected peers).
            // Also reject new peers that lost address conflicts or with existing peers

            let mut peers_to_remove = Vec::new();
            for (peer_id, peer_info) in state_guard.known_peers.iter() {
                // Skip connected peers as we don't want to disconnect them anyway
                if state_guard.connected_peers.contains_key(peer_id) {
                    continue;
                }

                for address in &peer_info.addresses {
                    if let Some(new_peer_info) = address_to_peer.get(address) {
                        // if the peer_id is the same, we will update it later
                        if new_peer_info.peer_id != *peer_id {
                            // Conflict detected
                            if new_peer_info.timestamp_ms > peer_info.timestamp_ms {
                                debug!(
                                    "Address conflict: newer peer {} replaces older peer {} for address {}",
                                    new_peer_info.peer_id, peer_id, address
                                );
                                // New peer wins - remove old peer
                                peers_to_remove.push(*peer_id);
                                if !peer_info.addresses.is_empty() {
                                    metrics.dec_num_peers_with_external_address();
                                }
                                break; // No need to check other addresses of this peer
                            } else {
                                // Existing peer wins - reject this new peer
                                debug!(
                                    "Address conflict: existing peer {} keeps address {} over older peer {}",
                                    peer_id, address, new_peer_info.peer_id
                                );
                                new_peers_to_reject.insert(new_peer_info.peer_id);
                            }
                        }
                    }
                }
            }

            // Remove the peers that lost conflicts
            for peer_id in peers_to_remove {
                state_guard.known_peers.remove(&peer_id);
            }
        }

        if !new_peers_to_reject.is_empty() {
            // Remove new peers that lost address conflicts
            verified_peers
                .retain(|(peer_info, _)| !new_peers_to_reject.contains(&peer_info.peer_id));
        }

        // Insert/update the remaining verified peers
        for (verified_peer, _) in verified_peers {
            insert_or_update_peer(&mut state_guard, verified_peer, &metrics);
        }
    }
}

/// Inserts or updates a peer in the known peers list.
/// If the peer already exists, its NodeInfo is updated only if the new one has
/// a newer timestamp.
fn insert_or_update_peer(
    state_guard: &mut std::sync::RwLockWriteGuard<'_, State>,
    peer: VerifiedSignedNodeInfo,
    metrics: &Metrics,
) {
    match state_guard.known_peers.entry(peer.peer_id) {
        // Updates the NodeInfo of the peer if it exists.
        Entry::Occupied(mut existing_entry) => {
            if peer.timestamp_ms > existing_entry.get().timestamp_ms {
                if existing_entry.get().addresses.is_empty() && !peer.addresses.is_empty() {
                    metrics.inc_num_peers_with_external_address();
                }
                if !existing_entry.get().addresses.is_empty() && peer.addresses.is_empty() {
                    metrics.dec_num_peers_with_external_address();
                }
                existing_entry.insert(peer);
            }
        }
        // Inserts the peer if it doesn't exist.
        Entry::Vacant(v) => {
            if !peer.addresses.is_empty() {
                metrics.inc_num_peers_with_external_address();
            }
            v.insert(peer);
        }
    }
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
