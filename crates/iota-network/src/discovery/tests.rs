// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, time::Duration};

use anemo::{Result, types::PeerAffinity};
use fastcrypto::{ed25519::Ed25519PublicKey, traits::KeyPair};
use futures::stream::FuturesUnordered;
use iota_config::{local_ip_utils, p2p::AllowlistedPeer};
use tokio::time::timeout;

use super::*;
use crate::utils::{
    build_network_and_key, build_network_with_address_and_anemo_config,
    build_network_with_anemo_config,
};

///////////////////////////////////////////////
// Helper functions for common test patterns //
///////////////////////////////////////////////

fn default_discovery_config_with_private_addresses_allowed() -> DiscoveryConfig {
    DiscoveryConfig {
        allow_private_addresses: Some(true),
        ..Default::default()
    }
}

fn default_p2p_config_with_private_addresses_allowed() -> P2pConfig {
    P2pConfig::default()
        .set_discovery_config(default_discovery_config_with_private_addresses_allowed())
}

fn assert_peers(
    self_name: &str,
    network: &Network,
    state: &Arc<RwLock<State>>,
    expected_network_known_peers: HashSet<PeerId>,
    expected_network_connected_peers: HashSet<PeerId>,
    expected_discovery_known_peers: HashSet<PeerId>,
    expected_discovery_connected_peers: HashSet<PeerId>,
) {
    let actual = network
        .known_peers()
        .get_all()
        .iter()
        .map(|pi| pi.peer_id)
        .collect::<HashSet<_>>();
    assert_eq!(
        actual, expected_network_known_peers,
        "{self_name} network known peers mismatch. Expected: {expected_network_known_peers:#?}, actual: {actual:#?}",
    );
    let actual = network.peers().iter().copied().collect::<HashSet<_>>();
    assert_eq!(
        actual, expected_network_connected_peers,
        "{self_name} network connected peers mismatch. Expected: {expected_network_connected_peers:#?}, actual: {actual:#?}",
    );
    let actual = state
        .read()
        .unwrap()
        .known_peers
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(
        actual, expected_discovery_known_peers,
        "{self_name} discovery known peers mismatch. Expected: {expected_discovery_known_peers:#?}, actual: {actual:#?}",
    );

    let actual = state
        .read()
        .unwrap()
        .connected_peers
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(
        actual, expected_discovery_connected_peers,
        "{self_name} discovery connected peers mismatch. Expected: {expected_discovery_connected_peers:#?}, actual: {actual:#?}",
    );
}

fn unwrap_new_peer_event(event: PeerEvent) -> PeerId {
    match event {
        PeerEvent::NewPeer(peer_id) => peer_id,
        e => panic!("unexpected event: {e:?}"),
    }
}

fn local_allowlisted_peer(peer_id: PeerId, port: Option<u16>) -> AllowlistedPeer {
    AllowlistedPeer {
        peer_id,
        address: port.map(|port| format!("/dns/localhost/udp/{port}").parse().unwrap()),
    }
}

fn set_up_network_with_trusted_peer_change_rx(
    p2p_config: P2pConfig,
    address: Option<anemo::types::Address>,
    trusted_peer_change_rx: watch::Receiver<TrustedPeerChangeEvent>,
) -> (
    UnstartedDiscovery,
    DiscoveryServer<impl Discovery>,
    Network,
    NetworkKeyPair,
) {
    let anemo_config = p2p_config.anemo_config.clone().unwrap_or_default();

    let (discovery, server) = Builder::new(trusted_peer_change_rx)
        .config(p2p_config)
        .build();

    let (network, keypair) = match address {
        Some(addr) => build_network_with_address_and_anemo_config(
            |router| router.add_rpc_service(server.clone()),
            addr,
            anemo_config,
        ),
        None => build_network_with_anemo_config(
            |router| router.add_rpc_service(server.clone()),
            anemo_config,
        ),
    };

    (discovery, server, network, keypair)
}

fn set_up_network(
    p2p_config: P2pConfig,
    address: Option<anemo::types::Address>,
) -> (
    UnstartedDiscovery,
    DiscoveryServer<impl Discovery>,
    Network,
    NetworkKeyPair,
) {
    set_up_network_with_trusted_peer_change_rx(p2p_config, address, create_test_channel().1)
}

fn start_network(
    discovery: UnstartedDiscovery,
    network: Network,
    keypair: NetworkKeyPair,
) -> (DiscoveryEventLoop, Handle, Arc<RwLock<State>>) {
    let (mut event_loop, handle) = discovery.build(network.clone(), keypair);
    event_loop.config.external_address = Some(local_multiaddr_from_network(&network));
    let state = event_loop.state.clone();
    (event_loop, handle, state)
}

fn start_network_without_external_address(
    discovery: UnstartedDiscovery,
    network: Network,
    keypair: NetworkKeyPair,
) -> (DiscoveryEventLoop, Handle, Arc<RwLock<State>>) {
    let (event_loop, handle) = discovery.build(network, keypair);
    let state = event_loop.state.clone();
    (event_loop, handle, state)
}

fn create_test_channel() -> (
    watch::Sender<TrustedPeerChangeEvent>,
    watch::Receiver<TrustedPeerChangeEvent>,
) {
    let (tx, rx) = watch::channel(TrustedPeerChangeEvent {
        new_committee: vec![],
        old_committee: vec![],
    });
    (tx, rx)
}

fn multiaddr_with_available_local_port(address_format: &str) -> Multiaddr {
    let port = local_ip_utils::get_available_port(&local_ip_utils::localhost_for_testing());
    address_format
        .replace("{}", &port.to_string())
        .parse()
        .unwrap()
}

/// Helper to create multiaddr from network's port
fn local_multiaddr_from_network(network: &Network) -> Multiaddr {
    format!("/dns/localhost/udp/{}", network.local_addr().port())
        .parse()
        .unwrap()
}

fn assert_known_peers_count(
    state: &Arc<RwLock<State>>,
    expected_count: usize,
    message_format: &str,
) {
    let state_guard = state.read().unwrap();
    assert_eq!(
        state_guard.known_peers.len(),
        expected_count,
        "{}",
        message_format.replace("{}", &state_guard.known_peers.len().to_string()),
    );
}

/// Helper to assert peer is in known_peers
fn assert_peer_in_known_peers(
    state: &Arc<RwLock<State>>,
    peer_id: &PeerId,
    should_be_known: bool,
    message_format: &str,
) {
    let state_guard = state.read().unwrap();
    if should_be_known {
        assert!(
            state_guard.known_peers.contains_key(peer_id),
            "{}",
            message_format.replace("{}", &peer_id.to_string()),
        );
    } else {
        assert!(
            !state_guard.known_peers.contains_key(peer_id),
            "{}",
            message_format.replace("{}", &peer_id.to_string()),
        );
    }
}

fn assert_known_peer_address(
    state: &Arc<RwLock<State>>,
    peer_id: &PeerId,
    expected_address: &Multiaddr,
    message_format: &str,
) {
    let state_guard = state.read().unwrap();

    if let Some(peer_info) = state_guard.known_peers.get(peer_id) {
        assert!(
            peer_info.addresses.contains(expected_address),
            "{}",
            message_format.replace("{}", &peer_id.to_string()),
        );
    } else {
        panic!("Peer {peer_id} not found in known_peers");
    }
}

/// Helper to assert peer is in cooldown
fn assert_peer_in_cooldown(
    state: &Arc<RwLock<State>>,
    peer_id: &PeerId,
    should_be_in_cooldown: bool,
    message_format: &str,
) {
    let state_guard = state.read().unwrap();
    if should_be_in_cooldown {
        assert!(
            state_guard
                .address_verification_cooldown
                .contains_key(peer_id),
            "{}",
            message_format.replace("{}", &peer_id.to_string()),
        );
    } else {
        assert!(
            !state_guard
                .address_verification_cooldown
                .contains_key(peer_id),
            "{}",
            message_format.replace("{}", &peer_id.to_string()),
        );
    }
}

/// Helper to manually expire a peer's cooldown for testing
fn expire_peer_cooldown(state: &Arc<RwLock<State>>, peer_id: &PeerId) {
    let mut state_guard = state.write().unwrap();
    if let Some(failure_time) = state_guard.address_verification_cooldown.get_mut(peer_id) {
        *failure_time = std::time::Instant::now() - Duration::from_secs(15 * 60); // 15 minutes ago
    }
}

/// Helper to update known peers and return the result for testing
async fn update_peers_for_test(
    network: &Network,
    state: Arc<RwLock<State>>,
    peers: Vec<SignedNodeInfo>,
    allow_private_addresses: bool,
) {
    let config = DiscoveryConfig {
        allow_private_addresses: Some(allow_private_addresses),
        ..Default::default()
    };
    update_known_peers(
        network,
        state,
        Metrics::disabled(),
        peers,
        Arc::new(HashMap::new()),
        &config,
    )
    .await;
}

///////////
// Tests //
///////////

#[tokio::test]
async fn get_known_peers() -> Result<()> {
    let config = default_p2p_config_with_private_addresses_allowed();

    let (discovery, server, network, key) = set_up_network(config, None);
    let key_for_signing = key.copy();
    let (_event_loop, _handle, state) = start_network(discovery, network.clone(), key);

    // Err when own_info not set
    server
        .inner()
        .get_known_peers_v2(Request::new(()))
        .await
        .unwrap_err();

    // Normal response with our_info
    let our_info = NodeInfo {
        peer_id: PeerId([9; 32]),
        addresses: Vec::new(),
        timestamp_ms: now_unix(),
        access_type: AccessType::Public,
    };

    let signed_our_info = our_info.clone().sign(&key_for_signing);
    state.write().unwrap().our_info = Some(signed_our_info);

    let response = server
        .inner()
        .get_known_peers_v2(Request::new(()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.own_info.data(), &our_info);
    assert!(response.known_peers.is_empty());

    // Normal response with some known peers
    let address_other: Multiaddr = "/dns/localhost/udp/1234".parse()?;
    let peer_info_other = NodeInfo {
        peer_id: PeerId([13; 32]),
        addresses: vec![address_other],
        timestamp_ms: now_unix(),
        access_type: AccessType::Public,
    };
    state.write().unwrap().known_peers.insert(
        peer_info_other.peer_id,
        VerifiedSignedNodeInfo::new_unchecked(SignedNodeInfo::new_from_data_and_sig(
            peer_info_other.clone(),
            Ed25519Signature::default(),
        )),
    );
    let response = server
        .inner()
        .get_known_peers_v2(Request::new(()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.own_info.data(), &our_info);
    assert_eq!(
        response
            .known_peers
            .into_iter()
            .map(|peer| peer.into_data())
            .collect::<Vec<_>>(),
        vec![peer_info_other]
    );

    Ok(())
}

#[tokio::test]
async fn make_connection_to_seed_peer() -> Result<()> {
    let mut config = default_p2p_config_with_private_addresses_allowed();

    let (discovery_1, _server_1, network_1, key_1) = set_up_network(config.clone(), None);
    let (_event_loop_1, _handle_1, _state_1) = start_network(discovery_1, network_1.clone(), key_1);

    config.seed_peers.push(SeedPeer {
        peer_id: None,
        address: local_multiaddr_from_network(&network_1),
    });

    let (discovery_2, _server_2, network_2, key_2) = set_up_network(config.clone(), None);
    let (mut event_loop_2, _handle_2, _state_2) =
        start_network(discovery_2, network_2.clone(), key_2);

    let (mut subscriber_1, _) = network_1.subscribe()?;
    let (mut subscriber_2, _) = network_2.subscribe()?;

    event_loop_2.handle_tick(std::time::Instant::now(), now_unix());

    assert_eq!(
        subscriber_2.recv().await?,
        PeerEvent::NewPeer(network_1.peer_id())
    );
    assert_eq!(
        subscriber_1.recv().await?,
        PeerEvent::NewPeer(network_2.peer_id())
    );

    Ok(())
}

#[tokio::test]
async fn make_connection_to_seed_peer_with_peer_id() -> Result<()> {
    let mut config = default_p2p_config_with_private_addresses_allowed();

    let (discovery_1, _server_1, network_1, key_1) = set_up_network(config.clone(), None);
    let (_event_loop_1, _handle_1, _state_1) = start_network(discovery_1, network_1.clone(), key_1);

    config.seed_peers.push(SeedPeer {
        peer_id: Some(network_1.peer_id()),
        address: local_multiaddr_from_network(&network_1),
    });

    let (discovery_2, _server_2, network_2, key_2) = set_up_network(config, None);
    let (mut event_loop_2, _handle_2, _state_2) =
        start_network(discovery_2, network_2.clone(), key_2);

    let (mut subscriber_1, _) = network_1.subscribe()?;
    let (mut subscriber_2, _) = network_2.subscribe()?;

    event_loop_2.handle_tick(std::time::Instant::now(), now_unix());

    assert_eq!(
        subscriber_2.recv().await?,
        PeerEvent::NewPeer(network_1.peer_id())
    );
    assert_eq!(
        subscriber_1.recv().await?,
        PeerEvent::NewPeer(network_2.peer_id())
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn three_nodes_can_connect_via_discovery() -> Result<()> {
    let mut config = default_p2p_config_with_private_addresses_allowed();

    // Setup the peer that will be the seed for the other two
    let (discovery_1, _server_1, network_1, key_1) = set_up_network(config.clone(), None);
    let (event_loop_1, _handle_1, _state_1) = start_network(discovery_1, network_1.clone(), key_1);

    config.seed_peers.push(SeedPeer {
        peer_id: Some(network_1.peer_id()),
        address: local_multiaddr_from_network(&network_1),
    });

    let (discovery_2, _server_2, network_2, key_2) = set_up_network(config.clone(), None);
    let (event_loop_2, _handle_2, _state_2) = start_network(discovery_2, network_2.clone(), key_2);

    let (discovery_3, _server_3, network_3, key_3) = set_up_network(config, None);
    let (event_loop_3, _handle_3, _state_3) = start_network(discovery_3, network_3.clone(), key_3);

    let (mut subscriber_1, _) = network_1.subscribe()?;
    let (mut subscriber_2, _) = network_2.subscribe()?;
    let (mut subscriber_3, _) = network_3.subscribe()?;

    // Start all the event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());
    tokio::spawn(event_loop_3.start());

    // advance the internal tokio time, so that the "handle_tick"'s are called
    tokio::time::sleep(Duration::from_secs(15)).await;

    let peer_id_1 = network_1.peer_id();
    let peer_id_2 = network_2.peer_id();
    let peer_id_3 = network_3.peer_id();

    let mut connected_peers_1 = Vec::new();
    let mut connected_peers_2 = Vec::new();
    let mut connected_peers_3 = Vec::new();

    // Collect all events from each subscriber and filter for NewPeer events
    tokio::time::timeout(Duration::from_secs(1), async {
        // Keep collecting until we have at least 2 NewPeer events per node or timeout
        loop {
            tokio::select! {
                event = subscriber_1.recv() => {
                    if let Ok(PeerEvent::NewPeer(peer_id)) = event {
                        connected_peers_1.push(peer_id);
                    }
                }
                event = subscriber_2.recv() => {
                    if let Ok(PeerEvent::NewPeer(peer_id)) = event {
                        connected_peers_2.push(peer_id);
                    }
                }
                event = subscriber_3.recv() => {
                    if let Ok(PeerEvent::NewPeer(peer_id)) = event {
                        connected_peers_3.push(peer_id);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if connected_peers_1.len() >= 2 && connected_peers_2.len() >= 2 && connected_peers_3.len() >= 2 {
                        break;
                    }
                }
            }
        }
    }).await?;

    // Check that each node is connected to the other two
    assert!(
        connected_peers_1.contains(&peer_id_2),
        "Node 1 should be connected to node 2"
    );
    assert!(
        connected_peers_1.contains(&peer_id_3),
        "Node 1 should be connected to node 3"
    );

    assert!(
        connected_peers_2.contains(&peer_id_1),
        "Node 2 should be connected to node 1"
    );
    assert!(
        connected_peers_2.contains(&peer_id_3),
        "Node 2 should be connected to node 3"
    );

    assert!(
        connected_peers_3.contains(&peer_id_1),
        "Node 3 should be connected to node 1"
    );
    assert!(
        connected_peers_3.contains(&peer_id_2),
        "Node 3 should be connected to node 2"
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn peers_are_added_from_reconfig_channel() -> Result<()> {
    let config = P2pConfig::default();

    let (tx_1, rx_1) = create_test_channel();

    let (discovery_1, _server_1, network_1, key_1) =
        set_up_network_with_trusted_peer_change_rx(config.clone(), None, rx_1);
    let (event_loop_1, _handle_1, _state_1) = start_network(discovery_1, network_1.clone(), key_1);

    let (discovery_2, _server_2, network_2, key_2) = set_up_network(config.clone(), None);
    let (event_loop_2, _handle_2, _state_2) = start_network(discovery_2, network_2.clone(), key_2);

    let (mut subscriber_1, _) = network_1.subscribe()?;
    let (mut subscriber_2, _) = network_2.subscribe()?;

    // Start all the event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());

    let peer_id_1 = network_1.peer_id();
    let peer_id_2 = network_2.peer_id();

    // At this moment peer 1 and peer 2 are not connected.
    let mut futures = FuturesUnordered::new();
    futures.push(timeout(Duration::from_secs(2), subscriber_1.recv()));
    futures.push(timeout(Duration::from_secs(2), subscriber_2.recv()));
    while let Some(result) = futures.next().await {
        let _elapse = result.unwrap_err();
    }

    let (mut subscriber_1, _) = network_1.subscribe()?;
    let (mut subscriber_2, _) = network_2.subscribe()?;

    // We send peer 1 a new peer info (peer 2) in the channel.
    let peer_2_network_pubkey =
        Ed25519PublicKey(ed25519_consensus::VerificationKey::try_from(peer_id_2.0).unwrap());
    let peer2_addr: Multiaddr = local_multiaddr_from_network(&network_2);
    tx_1.send(TrustedPeerChangeEvent {
        new_committee: vec![PeerInfo {
            peer_id: PeerId(peer_2_network_pubkey.0.to_bytes()),
            affinity: PeerAffinity::High,
            address: vec![peer2_addr.to_anemo_address().unwrap()],
        }],
        old_committee: vec![],
    })
    .unwrap();

    // Now peer 1 and peer 2 are connected.
    let new_peer_for_1 = unwrap_new_peer_event(subscriber_1.recv().await.unwrap());
    assert_eq!(new_peer_for_1, peer_id_2);
    let new_peer_for_2 = unwrap_new_peer_event(subscriber_2.recv().await.unwrap());
    assert_eq!(new_peer_for_2, peer_id_1);

    Ok(())
}

#[tokio::test]
async fn test_access_types() {
    // This test case constructs a mesh graph of 11 nodes, with the following
    // topology. For allowlisted nodes, `+` means the peer is allowlisted with
    // an address, otherwise not. An allowlisted peer with address will be
    // proactively connected in anemo network.
    //
    //
    // The topology:
    //
    //    11 (private, seed: 1, allowed: 7, 8)
    //                             \
    //                       ------ 1 (public) ------
    //                      /                        \
    //     2 (public, seed: 1, allowed: 7, 8)        |
    //       |                                       /
    //       |                          3 (private, seed: 1, allowed: 4+, 5+)
    //       |                         /             \
    //       |   4 (private, allowed: 3+, 5, 6) --- 5 (private, allowed: 3, 4+)
    //       |                         \
    //       |                       6 (private, allowed: 4+)
    //       |
    //     7 (private, allowed: 2+, 8+)
    //       |
    //       |
    //     8 (private, allowed: 7+, 9+)  p.s. 8's max connection is 0
    //       |
    //       |
    //     9 (public)
    //       |
    //       |
    //    10 (private, seed: 9)

    telemetry_subscribers::init_for_testing();

    let default_discovery_config = DiscoveryConfig {
        target_concurrent_connections: Some(100),
        interval_period_ms: Some(1000),
        allow_private_addresses: Some(true), // Allow localhost for testing
        ..Default::default()
    };
    let default_p2p_config = P2pConfig {
        discovery: Some(default_discovery_config.clone()),
        ..Default::default()
    };
    let default_private_discovery_config = DiscoveryConfig {
        target_concurrent_connections: Some(100),
        interval_period_ms: Some(1000),
        access_type: Some(AccessType::Private),
        allow_private_addresses: Some(true), // Allow localhost for testing
        ..Default::default()
    };

    // Node 1, public
    let (discovery_1, _server_1, network_1, key_1) =
        set_up_network(default_p2p_config.clone(), None);

    let mut config = default_p2p_config.clone();
    config.seed_peers.push(SeedPeer {
        peer_id: Some(network_1.peer_id()),
        address: local_multiaddr_from_network(&network_1),
    });

    // Node 2, public, seed: Node 1, allowlist: Node 7, Node 8
    let (mut discovery_2, _server_2, network_2, key_2) = set_up_network(config.clone(), None);

    // Node 3, private, seed: Node 1
    let (mut discovery_3, _server_3, network_3, key_3) = set_up_network(config.clone(), None);

    // Node 4, private, allowlist: Node 3, 5, and 6
    let (mut discovery_4, _server_4, network_4, key_4) =
        set_up_network(default_p2p_config_with_private_addresses_allowed(), None);

    // Node 5, private, allowlisted: Node 3 and Node 4
    let (discovery_5, _server_5, network_5, key_5) = {
        let mut private_discovery_config = default_private_discovery_config.clone();
        private_discovery_config.allowlisted_peers = vec![
            // Initially 5 does not know how to contact 3 or 4.
            local_allowlisted_peer(network_3.peer_id(), None),
            local_allowlisted_peer(network_4.peer_id(), Some(network_4.local_addr().port())),
        ];
        set_up_network(
            P2pConfig::default().set_discovery_config(private_discovery_config),
            None,
        )
    };

    // Node 6, private, allowlisted: Node 4
    let (discovery_6, _server_6, network_6, key_6) = {
        let mut private_discovery_config = default_private_discovery_config.clone();
        private_discovery_config.allowlisted_peers = vec![local_allowlisted_peer(
            network_4.peer_id(),
            Some(network_4.local_addr().port()),
        )];
        set_up_network(
            P2pConfig::default().set_discovery_config(private_discovery_config),
            None,
        )
    };

    // Node 3: Add Node 4 and Node 5 to allowlist
    let mut private_discovery_config = default_private_discovery_config.clone();
    private_discovery_config.allowlisted_peers = vec![
        local_allowlisted_peer(network_4.peer_id(), Some(network_4.local_addr().port())),
        local_allowlisted_peer(network_5.peer_id(), Some(network_5.local_addr().port())),
    ];
    discovery_3.config.discovery = Some(private_discovery_config);

    // Node 4: Add Node 3, Node 5, and Node 6 to allowlist
    let mut private_discovery_config = default_private_discovery_config.clone();
    private_discovery_config.allowlisted_peers = vec![
        local_allowlisted_peer(network_3.peer_id(), Some(network_3.local_addr().port())),
        local_allowlisted_peer(network_5.peer_id(), None),
        local_allowlisted_peer(network_6.peer_id(), None),
    ];
    discovery_4.config.discovery = Some(private_discovery_config);

    // Node 7, private, allowlisted: Node 2, Node 8
    let (mut discovery_7, _server_7, network_7, key_7) = set_up_network(
        P2pConfig::default().set_discovery_config(default_private_discovery_config.clone()),
        None,
    );

    // Node 9, public
    let (discovery_9, _server_9, network_9, key_9) =
        set_up_network(default_p2p_config.clone(), None);

    // Node 8, private, allowlisted: Node 7, Node 9
    let (discovery_8, _server_8, network_8, key_8) = {
        let mut private_discovery_config = default_private_discovery_config.clone();
        private_discovery_config.allowlisted_peers = vec![
            local_allowlisted_peer(network_7.peer_id(), Some(network_7.local_addr().port())),
            local_allowlisted_peer(network_9.peer_id(), Some(network_9.local_addr().port())),
        ];
        let mut p2p_config = P2pConfig::default();
        let mut anemo_config = anemo::Config::default();
        anemo_config.max_concurrent_connections = Some(0);
        p2p_config.anemo_config = Some(anemo_config);
        set_up_network(
            p2p_config.set_discovery_config(private_discovery_config),
            None,
        )
    };

    // Node 2, Add Node 7 and Node 8 to allowlist
    let mut discovery_config = default_discovery_config.clone();
    discovery_config.allowlisted_peers = vec![
        local_allowlisted_peer(network_7.peer_id(), None),
        local_allowlisted_peer(network_8.peer_id(), None),
    ];
    discovery_2.config.discovery = Some(discovery_config);

    // Node 7: Add Node 2, and Node 8 to allowlist
    let mut private_discovery_config = default_private_discovery_config.clone();
    private_discovery_config.allowlisted_peers = vec![
        local_allowlisted_peer(network_2.peer_id(), Some(network_2.local_addr().port())),
        local_allowlisted_peer(network_8.peer_id(), Some(network_8.local_addr().port())),
    ];
    discovery_7.config.discovery = Some(private_discovery_config);

    // Node 10, private, seed: 9
    let (discovery_10, _server_10, network_10, key_10) = {
        let mut p2p_config = default_p2p_config.clone();
        p2p_config.seed_peers.push(SeedPeer {
            peer_id: Some(network_9.peer_id()),
            address: local_multiaddr_from_network(&network_9),
        });
        p2p_config.discovery = Some(default_private_discovery_config.clone());
        set_up_network(p2p_config.clone(), None)
    };

    // Node 11, private, seed: 1, allow: 7, 8
    let (discovery_11, _server_11, network_11, key_11) = {
        let mut p2p_config = default_p2p_config.clone();
        p2p_config.seed_peers.push(SeedPeer {
            peer_id: Some(network_1.peer_id()),
            address: local_multiaddr_from_network(&network_1),
        });
        let mut private_discovery_config = default_private_discovery_config.clone();
        private_discovery_config.allowlisted_peers = vec![
            local_allowlisted_peer(network_8.peer_id(), None),
            local_allowlisted_peer(network_7.peer_id(), None),
        ];
        p2p_config.discovery = Some(private_discovery_config);
        set_up_network(p2p_config, None)
    };

    let (event_loop_1, _handle_1, state_1) = start_network(discovery_1, network_1.clone(), key_1);
    let (event_loop_2, _handle_2, state_2) = start_network(discovery_2, network_2.clone(), key_2);
    let (event_loop_3, _handle_3, state_3) = start_network(discovery_3, network_3.clone(), key_3);
    let (event_loop_4, _handle_4, state_4) = start_network(discovery_4, network_4.clone(), key_4);
    let (event_loop_5, _handle_5, state_5) = start_network(discovery_5, network_5.clone(), key_5);
    let (event_loop_6, _handle_6, state_6) = start_network(discovery_6, network_6.clone(), key_6);
    let (event_loop_7, _handle_7, state_7) = start_network(discovery_7, network_7.clone(), key_7);
    let (event_loop_8, _handle_8, state_8) = start_network(discovery_8, network_8.clone(), key_8);
    let (event_loop_9, _handle_9, state_9) = start_network(discovery_9, network_9.clone(), key_9);
    let (event_loop_10, _handle_10, state_10) =
        start_network(discovery_10, network_10.clone(), key_10);
    let (event_loop_11, _handle_11, state_11) =
        start_network(discovery_11, network_11.clone(), key_11);

    // Start all the event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());
    tokio::spawn(event_loop_3.start());
    tokio::spawn(event_loop_4.start());
    tokio::spawn(event_loop_5.start());
    tokio::spawn(event_loop_6.start());
    tokio::spawn(event_loop_7.start());
    tokio::spawn(event_loop_8.start());
    tokio::spawn(event_loop_9.start());
    tokio::spawn(event_loop_10.start());
    tokio::spawn(event_loop_11.start());

    let peer_id_1 = network_1.peer_id();
    let peer_id_2 = network_2.peer_id();
    let peer_id_3 = network_3.peer_id();
    let peer_id_4 = network_4.peer_id();
    let peer_id_5 = network_5.peer_id();
    let peer_id_6 = network_6.peer_id();
    let peer_id_7 = network_7.peer_id();
    let peer_id_8 = network_8.peer_id();
    let peer_id_9 = network_9.peer_id();
    let peer_id_10 = network_10.peer_id();
    let peer_id_11 = network_11.peer_id();

    info!("peer_id_1: {:?}", peer_id_1);
    info!("peer_id_2: {:?}", peer_id_2);
    info!("peer_id_3: {:?}", peer_id_3);
    info!("peer_id_4: {:?}", peer_id_4);
    info!("peer_id_5: {:?}", peer_id_5);
    info!("peer_id_6: {:?}", peer_id_6);
    info!("peer_id_7: {:?}", peer_id_7);
    info!("peer_id_8: {:?}", peer_id_8);
    info!("peer_id_9: {:?}", peer_id_9);
    info!("peer_id_10: {:?}", peer_id_10);
    info!("peer_id_11: {:?}", peer_id_11);

    // Let them fully connect
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Node 1 is connected to everyone. But it does not "know" private nodes.
    assert_peers(
        "Node 1",
        &network_1,
        &state_1,
        HashSet::from_iter(vec![]),
        HashSet::from_iter(vec![
            peer_id_2, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8, peer_id_9,
            peer_id_10, peer_id_11,
        ]),
        HashSet::from_iter(vec![peer_id_2, peer_id_9]),
        HashSet::from_iter(vec![
            peer_id_2, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8, peer_id_9,
            peer_id_10, peer_id_11,
        ]),
    );

    // Node 2 is connected to everyone. But it does not "know" private nodes except
    // the allowlisted ones 7 and 8.
    assert_peers(
        "Node 2",
        &network_2,
        &state_2,
        HashSet::from_iter(vec![peer_id_1, peer_id_7, peer_id_8]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8, peer_id_9,
            peer_id_10, peer_id_11,
        ]),
        HashSet::from_iter(vec![peer_id_1, peer_id_7, peer_id_8, peer_id_9]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8, peer_id_9,
            peer_id_10, peer_id_11,
        ]),
    );

    assert_peers(
        "Node 3",
        &network_3,
        &state_3,
        HashSet::from_iter(vec![peer_id_1, peer_id_4, peer_id_5]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_5, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_5, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_5, peer_id_9]),
    );

    assert_peers(
        "Node 4",
        &network_4,
        &state_4,
        HashSet::from_iter(vec![peer_id_3, peer_id_5, peer_id_6]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_2, peer_id_3, peer_id_5, peer_id_6, peer_id_9,
        ]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_2, peer_id_3, peer_id_5, peer_id_6, peer_id_9,
        ]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_2, peer_id_3, peer_id_5, peer_id_6, peer_id_9,
        ]),
    );

    assert_peers(
        "Node 5",
        &network_5,
        &state_5,
        HashSet::from_iter(vec![peer_id_3, peer_id_4]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_3, peer_id_4, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_3, peer_id_4, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_3, peer_id_4, peer_id_9]),
    );

    assert_peers(
        "Node 6",
        &network_6,
        &state_6,
        HashSet::from_iter(vec![peer_id_4]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_4, peer_id_9]),
    );

    // Node 11 can't find private Node 7 via Node 2.
    assert_peers(
        "Node 7",
        &network_7,
        &state_7,
        HashSet::from_iter(vec![peer_id_2, peer_id_8]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_8, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_8, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_8, peer_id_9]),
    );

    // Node 11 can't find private Node 8 via Node 2.
    assert_peers(
        "Node 8",
        &network_8,
        &state_8,
        HashSet::from_iter(vec![peer_id_7, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_7, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_7, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_7, peer_id_9]),
    );

    assert_peers(
        "Node 9",
        &network_9,
        &state_9,
        HashSet::from_iter(vec![]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_2, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8,
            peer_id_10, peer_id_11,
        ]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2]),
        HashSet::from_iter(vec![
            peer_id_1, peer_id_2, peer_id_3, peer_id_4, peer_id_5, peer_id_6, peer_id_7, peer_id_8,
            peer_id_10, peer_id_11,
        ]),
    );

    // Node 10 does not talk to any other private nodes.
    assert_peers(
        "Node 10",
        &network_10,
        &state_10,
        HashSet::from_iter(vec![peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
    );

    // 11 allowlists 8 but 8 does not 11, so they can't connect
    // although 8 is still in 11's known peer list
    assert_peers(
        "Node 11",
        &network_11,
        &state_11,
        HashSet::from_iter(vec![peer_id_1, peer_id_7, peer_id_8]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
        HashSet::from_iter(vec![peer_id_1, peer_id_2, peer_id_9]),
    );
}

#[tokio::test]
async fn test_handle_trusted_peer_change_event() -> Result<()> {
    fn mock_multiaddr(port: u16) -> Multiaddr {
        format!("/dns/mock.local/udp/{port}").parse().unwrap()
    }

    // Create mock peers, good enough for the test
    let peers: Vec<_> = (0..=5)
        .map(|id: u8| -> PeerInfo {
            PeerInfo {
                peer_id: PeerId([id; 32]),
                affinity: PeerAffinity::High,
                address: vec![mock_multiaddr(id as u16).to_anemo_address().unwrap()],
            }
        })
        .collect();

    // Updated peers have extra address
    let updated_peers: Vec<_> = peers
        .iter()
        .enumerate()
        .map(|(i, peer_info)| {
            let mut updated_peer_info = peer_info.clone();
            let addr = mock_multiaddr((i + 10) as u16).to_anemo_address().unwrap();
            updated_peer_info.address.push(addr);
            updated_peer_info
        })
        .collect();

    // Configure allowlisted and seed peers which should always be known
    let discovery_config = DiscoveryConfig {
        allowlisted_peers: vec![AllowlistedPeer {
            peer_id: peers[1].peer_id,
            address: Some(mock_multiaddr(1)),
        }],
        allow_private_addresses: Some(true), // Allow localhost for testing
        ..Default::default()
    };
    let mut config = P2pConfig::default().set_discovery_config(discovery_config);
    config.seed_peers = vec![SeedPeer {
        peer_id: Some(peers[2].peer_id),
        address: mock_multiaddr(2),
    }];

    // Setup test network and discovery event loop
    let (tx, mut rx) = create_test_channel();
    let (discovery, _server, network, key) =
        set_up_network_with_trusted_peer_change_rx(config.clone(), None, rx.clone());
    let (event_loop, _handle, _state) = start_network(discovery, network.clone(), key);

    let mut peer0 = peers[0].clone();
    peer0.peer_id = network.peer_id();
    peer0.address = vec![
        mock_multiaddr(network.local_addr().port())
            .to_anemo_address()
            .unwrap(),
    ];

    // This is how the event is sent in iota-node
    let send_trusted_peer_change = |new_committee| {
        tx.send_modify(|event| {
            core::mem::swap(&mut event.new_committee, &mut event.old_committee);
            event.new_committee = new_committee;
        })
    };

    // Start peer0 discovery event loop
    tokio::spawn(event_loop.start());

    // Iteration #1

    // The initial committee
    send_trusted_peer_change(vec![peer0.clone(), peers[3].clone(), peers[4].clone()]);

    // Wait for the event loop to handle the update, 2 sec should be enough
    rx.changed().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify known peers: 1,2,3,4
    let mut known_peers = network.known_peers().get_all();
    known_peers.sort_by_key(|peer_info| peer_info.peer_id);
    assert_eq!(known_peers[0].peer_id, peers[1].peer_id); // allowlisted
    assert_eq!(known_peers[1].peer_id, peers[2].peer_id); // seed peer
    assert_eq!(known_peers[2].peer_id, peers[3].peer_id); // new committee
    assert_eq!(known_peers[3].peer_id, peers[4].peer_id); // new committee

    // Iteration #2

    // The second committee
    send_trusted_peer_change(vec![peers[4].clone(), peers[5].clone()]);

    // Wait for the event loop to handle the update, 2 sec should be enough
    rx.changed().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify known peers: 1,2,4,5
    let mut known_peers = network.known_peers().get_all();
    known_peers.sort_by_key(|peer_info| peer_info.peer_id);
    assert_eq!(known_peers[0].peer_id, peers[1].peer_id); // allowlisted
    assert_eq!(known_peers[1].peer_id, peers[2].peer_id); // seed peer
    assert_eq!(known_peers[2].peer_id, peers[4].peer_id); // new committee
    assert_eq!(known_peers[3].peer_id, peers[5].peer_id); // new committee

    // Iteration #3

    // The third committee
    send_trusted_peer_change(vec![
        peer0.clone(),
        updated_peers[1].clone(),
        updated_peers[3].clone(),
        updated_peers[5].clone(),
    ]);

    // Wait for the event loop to handle the update, 2 sec should be enough
    rx.changed().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify known peers: 1*,2,3*,5*
    let mut known_peers = network.known_peers().get_all();
    known_peers.sort_by_key(|peer_info| peer_info.peer_id);
    assert_eq!(known_peers[0].peer_id, updated_peers[1].peer_id); // allowlisted and updated
    assert_eq!(known_peers[0].address.len(), 2);
    assert_eq!(known_peers[1].peer_id, peers[2].peer_id); // seed peer
    assert_eq!(known_peers[2].peer_id, updated_peers[3].peer_id); // new committee and updated
    assert_eq!(known_peers[2].address.len(), 2);
    assert_eq!(known_peers[3].peer_id, updated_peers[5].peer_id); // old committee and updated
    assert_eq!(known_peers[3].address.len(), 2);

    // Iteration #4

    // The next committee
    send_trusted_peer_change(vec![peer0.clone()]);

    // Wait for the event loop to handle the update, 2 sec should be enough
    rx.changed().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify known peers: 1*,2
    let mut known_peers = network.known_peers().get_all();
    known_peers.sort_by_key(|peer_info| peer_info.peer_id);
    assert_eq!(known_peers[0].peer_id, updated_peers[1].peer_id); // allowlisted and updated
    assert_eq!(known_peers[0].address.len(), 2);
    assert_eq!(known_peers[1].peer_id, peers[2].peer_id); // seed peer

    Ok(())
}

#[tokio::test]
async fn test_address_spoofing_prevention() -> Result<()> {
    // This test verifies that our address verification prevents malicious actors
    // from spamming the discovery with multiple peer entries using the same address
    // but different private keys, or with non-existent addresses.

    let config = P2pConfig::default();

    let (discovery_victim, _server_victim, network_victim, key_victim) =
        set_up_network(config.clone(), None);
    let (event_loop_victim, _handle_victim, _state_victim) =
        start_network(discovery_victim, network_victim.clone(), key_victim);

    // Start the victim node discovery event loop (this is the node that will
    // receive the attack)
    tokio::spawn(event_loop_victim.start());

    // Create a legitimate node that will be spoofed
    let (discovery_legitimate, _server_legitimate, network_legitimate, key_legitimate) =
        set_up_network(config.clone(), None);
    let key_legitimate_for_signing = key_legitimate.copy(); // Keep a copy for signing
    let (mut event_loop_legitimate, _handle_legitimate, _state_legitimate) = start_network(
        discovery_legitimate,
        network_legitimate.clone(),
        key_legitimate,
    );

    // Set an external address for the legitimate node - use the actual listening
    // address
    let address_legitimate = local_multiaddr_from_network(&network_legitimate);
    event_loop_legitimate.config.external_address = Some(address_legitimate.clone());

    // Start the legitimate node discovery event loop
    tokio::spawn(event_loop_legitimate.start());

    // Wait for nodes to start up
    tokio::time::sleep(Duration::from_millis(100)).await;

    let start_timestamp_ms = now_unix();

    // Create legitimate NodeInfo - use the same key that was used to create the
    // network
    let signed_peer_info_legitimate = NodeInfo {
        peer_id: network_legitimate.peer_id(),
        addresses: vec![address_legitimate.clone()],
        timestamp_ms: start_timestamp_ms,
        access_type: AccessType::Public,
    }
    .sign(&key_legitimate_for_signing); // Use the actual network key, not a random one

    // ATTACK VECTOR 1: Multiple malicious entries with the SAME address but
    // different peer IDs. This simulates the attack where a malicious actor
    // creates multiple "fake" nodes claiming to be at the same address but with
    // different valid keys
    let mut malicious_peers = Vec::new();

    for i in 0..5 {
        // Create different keypairs (different private keys) for each malicious peer
        let malicious_key = NetworkKeyPair::generate(&mut rand::thread_rng());
        let malicious_peer_id =
            anemo::PeerId(malicious_key.public().as_bytes().try_into().unwrap());
        let timestamp_malicious = start_timestamp_ms + i + 100;

        let signed_peer_info_malicious = NodeInfo {
            peer_id: malicious_peer_id,
            addresses: vec![address_legitimate.clone()], // SAME address as legitimate node
            timestamp_ms: timestamp_malicious,           /* Make sure these have newer timestamps
                                                          * than legitimate */
            access_type: AccessType::Public,
        }
        .sign(&malicious_key); // Sign with the matching key to pass signature verification

        malicious_peers.push(signed_peer_info_malicious);
    }

    // ATTACK VECTOR 2: Peer ID spoofing with non-existent address
    // Malicious actor claims to be a legitimate peer but at a fake
    // non-existing/non-reachable address
    let fake_address: Multiaddr = "/dns/localhost/udp/54321".parse()?;
    let key_malicious_2 = NetworkKeyPair::generate(&mut rand::thread_rng());
    let peer_id_malicious_2 =
        anemo::PeerId(key_malicious_2.public().as_bytes().try_into().unwrap());
    let timestamp_malicious_2 = start_timestamp_ms + 1000; // Newer timestamp
    let signed_peer_info_spoof_fake_addr = NodeInfo {
        peer_id: peer_id_malicious_2,
        addresses: vec![fake_address.clone()], // non-existent address
        timestamp_ms: timestamp_malicious_2,
        access_type: AccessType::Public,
    }
    .sign(&key_malicious_2);

    malicious_peers.push(signed_peer_info_spoof_fake_addr);

    // ATTACK VECTOR 3: Peer ID spoofing with real existing address of another node
    // Create another legitimate node to use as the "malicious_address"
    let (discovery_malicious_3, _server_malicious_3, network_malicious_3, key_malicious_3) =
        set_up_network(config.clone(), None);
    let key_malicious_3_for_signing = key_malicious_3.copy(); // Keep a copy for signing
    let (event_loop_malicious_3, _handle_malicious_3) =
        discovery_malicious_3.build(network_malicious_3.clone(), key_malicious_3);

    // Start the malicious node discovery event loop
    tokio::spawn(event_loop_malicious_3.start());

    // Wait for it to start up
    tokio::time::sleep(Duration::from_millis(100)).await;

    let timestamp_malicious_3 = start_timestamp_ms + 2000; // Even newer timestamp
    let address_malicious_3 = local_multiaddr_from_network(&network_malicious_3);
    let signed_peer_info_spoof_real_addr = NodeInfo {
        peer_id: network_legitimate.peer_id(), // SAME peer ID as legitimate peer!
        addresses: vec![address_malicious_3.clone()], // But address of malicious_address
        timestamp_ms: timestamp_malicious_3,   // Even newer timestamp
        access_type: AccessType::Public,
    }
    .sign(&key_malicious_3_for_signing); // Signed with wrong key - this should fail signature verification

    malicious_peers.push(signed_peer_info_spoof_real_addr);

    // Get the victim's state before the attack
    let (discovery_victim, _server_victim, network_victim, key_victim) =
        set_up_network(config, None);

    // Set up the victim's own info to avoid unwrap panics
    let signed_peer_info_victim = NodeInfo {
        peer_id: network_victim.peer_id(),
        addresses: Vec::new(),
        timestamp_ms: start_timestamp_ms,
        access_type: AccessType::Public,
    }
    .sign(&key_victim);

    let (_event_loop_victim, _handle_victim, state_victim) =
        start_network(discovery_victim, network_victim.clone(), key_victim);
    state_victim.write().unwrap().our_info = Some(signed_peer_info_victim);

    // Simulate what happens when the victim receives these peers through discovery
    // This would normally happen when a malicious node sends these as "known peers"
    let mut attack_peers = vec![signed_peer_info_legitimate];
    attack_peers.extend(malicious_peers.clone());

    update_peers_for_test(&network_victim, state_victim.clone(), attack_peers, true).await;

    // Verify that address deduplication and verification work together correctly
    let known_peers = state_victim.read().unwrap().known_peers.clone();

    // Address verification should reject malicious peers (wrong peer ID at address,
    // non-existent address) and keep verified ones only
    assert_eq!(
        known_peers.len(),
        1,
        "Should have exactly 1 peer (the legitimate one) after filtering out malicious peers. Found {} peers: {:?}",
        known_peers.len(),
        known_peers.keys().collect::<Vec<_>>()
    );

    // Verify it's the legitimate peer that survived
    assert!(
        known_peers.contains_key(&network_legitimate.peer_id()),
        "The legitimate peer should be the one that survived"
    );

    // Check that if the legitimate peer ID is in known_peers, it has the correct
    // address
    if let Some(surviving_peer) = known_peers.get(&network_legitimate.peer_id()) {
        assert_eq!(
            surviving_peer.addresses,
            vec![address_legitimate.clone()],
            "The surviving peer should have the legitimate address, not a spoofed one"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_address_conflict_resolution_with_existing_peers() -> Result<()> {
    // Test that address conflicts between new peers and existing entries are
    // resolved correctly

    let config = default_p2p_config_with_private_addresses_allowed();

    let start_timestamp_ms = now_unix();
    let timestamp_peer_1 = start_timestamp_ms + 100;
    let timestamp_peer_2 = start_timestamp_ms + 200;
    let timestamp_peer_3 = start_timestamp_ms + 150; // Older than 2 but newer than 1

    // Setup a network to act as the victim receiving conflicting peer info
    let address_victim: Multiaddr = multiaddr_with_available_local_port("/dns/localhost/udp/{}");
    let (discovery_victim, _server_victim, network_victim, key_victim) = set_up_network(
        config.clone(),
        Some(address_victim.clone().to_anemo_address().unwrap()),
    );

    let signed_peer_info_victim = NodeInfo {
        peer_id: network_victim.peer_id(),
        addresses: vec![address_victim],
        timestamp_ms: start_timestamp_ms,
        access_type: AccessType::Public,
    }
    .sign(&key_victim);

    let (_event_loop_victim, _handle_victim, state_victim) =
        start_network(discovery_victim, network_victim.clone(), key_victim);
    state_victim.write().unwrap().our_info = Some(signed_peer_info_victim);

    // Create network 1 first
    let (discovery_1, _server_1, network_1, key_1) = set_up_network(config.clone(), None);
    let key_1_for_signing = key_1.copy();
    let (event_loop_1, handle_1, _state_1) = start_network(discovery_1, network_1.clone(), key_1);
    let peer_id_1 = network_1.peer_id();

    // Get the address that network 1 is using
    let shared_socket_addr = network_1.local_addr();
    let shared_address = local_multiaddr_from_network(&network_1);

    let signed_peer_info_1 = NodeInfo {
        peer_id: peer_id_1,
        addresses: vec![shared_address.clone()],
        timestamp_ms: timestamp_peer_1,
        access_type: AccessType::Public,
    }
    .sign(&key_1_for_signing);

    // Add peer 1 to state using normal update process (should pass verification)
    update_peers_for_test(
        &network_victim,
        state_victim.clone(),
        vec![signed_peer_info_1],
        true,
    )
    .await;

    // Verify peer 1 was added
    {
        assert_known_peers_count(
            &state_victim,
            1,
            "Should have exactly 1 peer after adding peer 1 but got {}",
        );
        assert_peer_in_known_peers(
            &state_victim,
            &peer_id_1,
            true,
            "Peer 1 should be added initially",
        );
        assert_known_peer_address(
            &state_victim,
            &peer_id_1,
            &shared_address,
            "Peer 1 should have the correct address",
        );
    }

    // Shutdown network 1 to free up the address
    drop(event_loop_1);
    drop(handle_1);
    drop(network_1);
    tokio::time::sleep(Duration::from_millis(300)).await; // Give it time to shut down

    // Create network 2 with the same address that 1 was using
    let (discovery_2, _server_2, network_2, key_2) =
        set_up_network(config.clone(), Some(shared_socket_addr.into()));
    let key_2_for_signing = key_2.copy();
    let (event_loop_2, handle_2, _state_2) = start_network(discovery_2, network_2.clone(), key_2);
    let peer_id_2 = network_2.peer_id();

    let signed_peer_info_2 = NodeInfo {
        peer_id: peer_id_2,
        addresses: vec![shared_address.clone()], // Same address as peer 1
        timestamp_ms: timestamp_peer_2,
        access_type: AccessType::Public,
    }
    .sign(&key_2_for_signing);

    // Add peer 2 - this should trigger conflict resolution
    update_peers_for_test(
        &network_victim,
        state_victim.clone(),
        vec![signed_peer_info_2],
        true,
    )
    .await;

    // Verify conflict resolution: peer 2 should remain (newer), peer 1 should be
    // removed
    {
        assert_known_peers_count(
            &state_victim,
            1,
            "Should have exactly 1 peer after conflict resolution but got {}",
        );
        assert_peer_in_known_peers(
            &state_victim,
            &peer_id_1,
            false,
            "Older peer 1 should be removed due to address conflict",
        );
        assert_peer_in_known_peers(
            &state_victim,
            &peer_id_2,
            true,
            "Newer peer 2 should be kept",
        );
        assert_known_peer_address(
            &state_victim,
            &peer_id_2,
            &shared_address,
            "Peer 2 should have the correct address",
        );
    }

    // Now test the reverse: add peer 3 with an older timestamp than 2 (should be
    // rejected) Shutdown network 2 first to free up the address for peer 3
    drop(event_loop_2);
    drop(handle_2);
    drop(network_2);
    tokio::time::sleep(Duration::from_millis(300)).await; // Give it time to shut down

    // Create network 3 with the same address
    let (discovery_3, _server_3, network_3, key_3) =
        set_up_network(config.clone(), Some(shared_socket_addr.into()));
    let key_3_for_signing = key_3.copy();
    let (_event_loop_3, _handle_3, _state_3) = start_network(discovery_3, network_3.clone(), key_3);
    let peer_id_3 = network_3.peer_id();

    let signed_peer_info_3 = NodeInfo {
        peer_id: peer_id_3,
        addresses: vec![shared_address.clone()], // Same address again
        timestamp_ms: timestamp_peer_3,
        access_type: AccessType::Public,
    }
    .sign(&key_3_for_signing);

    // Add peer 3 - this should be rejected since 2 (newer) is already present
    update_peers_for_test(
        &network_victim,
        state_victim.clone(),
        vec![signed_peer_info_3],
        true,
    )
    .await;

    // Verify that peer 2 is still there and peer 3 was rejected
    {
        assert_known_peers_count(
            &state_victim,
            1,
            "Should still have exactly 1 peer after attempting to add older peer 3 but got {}",
        );
        assert_peer_in_known_peers(
            &state_victim,
            &peer_id_2,
            true,
            "Newer peer 2 should still be kept",
        );
        assert_peer_in_known_peers(
            &state_victim,
            &peer_id_3,
            false,
            "Older peer 3 should be rejected due to address conflict",
        );
    }

    Ok(())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_address_verification_cooldown_and_cleanup() -> Result<()> {
    // This test checks the address verification cooldown mechanism.

    // Create a test network and state
    let (discovery, _server, network, key) = set_up_network(P2pConfig::default(), None);

    // Set up the nodes's own info to avoid unwrap panics
    let signed_peer_info = NodeInfo {
        peer_id: network.peer_id(),
        addresses: Vec::new(),
        timestamp_ms: now_unix(),
        access_type: AccessType::Public,
    }
    .sign(&key);

    let (event_loop, _handle, state) = start_network(discovery, network.clone(), key);
    state.write().unwrap().our_info = Some(signed_peer_info);

    // Start the event loop to handle cleanup intervals
    tokio::spawn(event_loop.start());

    // Create a peer with an unreachable address that will fail verification
    let (network_other, key_other) = build_network_and_key(|router| router);
    let peer_id_other = network_other.peer_id();

    // Step 1: Add peer with unreachable address, check that it's in cooldown
    let peer_info_other = NodeInfo {
        peer_id: peer_id_other,
        addresses: vec![
            "/dns/unreachable-test-peer.invalid/udp/12345"
                .parse()
                .unwrap(),
        ], // Invalid domain, safe for testing
        timestamp_ms: now_unix(),
        access_type: AccessType::Public,
    };
    let signed_peer_info_other = peer_info_other.sign(&key_other);

    update_peers_for_test(&network, state.clone(), vec![signed_peer_info_other], true).await;

    // Verify peer is in cooldown after failed verification
    assert_peer_in_known_peers(
        &state,
        &peer_id_other,
        false,
        "Peer should not be in known_peers after failed verification",
    );
    assert_peer_in_cooldown(
        &state,
        &peer_id_other,
        true,
        "Peer should be in verification failure cooldown",
    );

    // Step 2: Re-add same peer with valid address (should be filtered by cooldown)
    let peer_info_valid_other = NodeInfo {
        peer_id: peer_id_other,                                        // Same peer ID
        addresses: vec![local_multiaddr_from_network(&network_other)], // Valid address
        timestamp_ms: now_unix() + 1000,                               // Newer timestamp
        access_type: AccessType::Public,
    };
    let signed_peer_info_valid_other = peer_info_valid_other.sign(&key_other);

    update_peers_for_test(
        &network,
        state.clone(),
        vec![signed_peer_info_valid_other.clone()],
        true,
    )
    .await;

    // Verify peer is still filtered out by cooldown
    assert_peer_in_known_peers(
        &state,
        &peer_id_other,
        false,
        "Peer should still not be in known_peers due to cooldown",
    );
    assert_peer_in_cooldown(
        &state,
        &peer_id_other,
        true,
        "Peer should be in verification failure cooldown",
    );

    // Step 3: Wait for cooldown to pass, re-add peer with valid address
    expire_peer_cooldown(&state, &peer_id_other);

    // Advance time to trigger the cleanup interval (default is 5 minutes = 300
    // seconds) Add a bit extra to ensure the cleanup runs
    tokio::time::sleep(Duration::from_secs(500)).await;

    update_peers_for_test(
        &network,
        state.clone(),
        vec![signed_peer_info_valid_other],
        true,
    )
    .await;

    // Verify peer is processed normally after cooldown expires
    assert_peer_in_known_peers(
        &state,
        &peer_id_other,
        true,
        "Peer should be in known_peers after cooldown",
    );
    assert_peer_in_cooldown(
        &state,
        &peer_id_other,
        false,
        "Peer should not be in verification failure cooldown",
    );

    Ok(())
}

#[tokio::test]
async fn test_peer_deduplication() -> Result<()> {
    // This test verifies that the deduplicate_peers function correctly handles
    // each field in NodeInfo individually

    // Create test keypairs
    let key1 = NetworkKeyPair::generate(&mut rand::thread_rng());
    let key2 = NetworkKeyPair::generate(&mut rand::thread_rng());

    let peer_id1 = anemo::PeerId(key1.public().as_bytes().try_into().unwrap());
    let peer_id2 = anemo::PeerId(key2.public().as_bytes().try_into().unwrap());

    let address1: Multiaddr = "/dns/localhost/udp/1111".parse()?;
    let address2: Multiaddr = "/dns/localhost/udp/2222".parse()?;

    let timestamp1 = now_unix();

    // Base peer info for testing
    let peer_info_base = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address1.clone()],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_base = peer_info_base.clone().sign(&key1);
    let signed_peer_base_different_key = peer_info_base.sign(&key2); // Same data, different signature

    let peer_info_other = NodeInfo {
        peer_id: peer_id2,
        addresses: vec![address2.clone()],
        timestamp_ms: timestamp1 + 1000,
        access_type: AccessType::Private,
    };
    let signed_peer_other = peer_info_other.sign(&key2);

    let peer_info_different_id = NodeInfo {
        peer_id: peer_id2, // Different peer ID
        addresses: vec![address1.clone()],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_different_id = peer_info_different_id.sign(&key1);

    let peer_info_different_address = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address2.clone()],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_different_address = peer_info_different_address.sign(&key1);

    let peer_info_multiple_addresses = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address1.clone(), address2.clone()],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_multiple_addresses = peer_info_multiple_addresses.sign(&key1);

    let peer_info_reordered_addresses = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address2, address1.clone()],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_reordered_addresses = peer_info_reordered_addresses.sign(&key1);

    let peer_info_empty_addresses = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![],
        timestamp_ms: timestamp1,
        access_type: AccessType::Public,
    };
    let signed_peer_empty_addresses = peer_info_empty_addresses.sign(&key1);

    let peer_info_different_timestamp = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address1.clone()],
        timestamp_ms: timestamp1 + 1000, // Different timestamp
        access_type: AccessType::Public,
    };
    let signed_peer_different_timestamp = peer_info_different_timestamp.sign(&key1);

    let peer_info_different_access_type = NodeInfo {
        peer_id: peer_id1,
        addresses: vec![address1],
        timestamp_ms: timestamp1,
        access_type: AccessType::Private, // Different access type
    };
    let signed_peer_different_access_type = peer_info_different_access_type.sign(&key1);

    // Test Case 1: Identical peers (same data, same signature) SHOULD be
    // deduplicated
    let identical_peers = vec![signed_peer_base.clone(), signed_peer_base.clone()];
    let result = deduplicate_peers(identical_peers);
    assert_eq!(
        result.len(),
        1,
        "Identical peers should be deduplicated to 1 peer"
    );

    // Test Case 2: Different peer_id field should NOT be deduplicated
    let different_id_peers = vec![signed_peer_base.clone(), signed_peer_different_id];
    let result = deduplicate_peers(different_id_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different peer_id should NOT be deduplicated"
    );

    // Test Case 3: Different single address should NOT be deduplicated
    let different_address_peers = vec![signed_peer_base.clone(), signed_peer_different_address];
    let result = deduplicate_peers(different_address_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different single address should NOT be deduplicated"
    );

    // Test Case 4: Different number of addresses should NOT be deduplicated
    let multiple_addresses_peers = vec![
        signed_peer_base.clone(),
        signed_peer_multiple_addresses.clone(),
    ];
    let result = deduplicate_peers(multiple_addresses_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different number of addresses should NOT be deduplicated"
    );

    // Test Case 5: Different address order should NOT be deduplicated
    let reordered_addresses_peers = vec![
        signed_peer_multiple_addresses,
        signed_peer_reordered_addresses,
    ];
    let result = deduplicate_peers(reordered_addresses_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different address order should NOT be deduplicated"
    );

    // Test Case 6: Empty addresses vs non-empty addresses should NOT be
    // deduplicated
    let empty_addresses_peers = vec![signed_peer_base.clone(), signed_peer_empty_addresses];
    let result = deduplicate_peers(empty_addresses_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with empty vs non-empty addresses should NOT be deduplicated"
    );

    // Test Case 7: Different timestamp_ms field should NOT be deduplicated
    let different_timestamp_peers = vec![
        signed_peer_base.clone(),
        signed_peer_different_timestamp.clone(),
    ];
    let result = deduplicate_peers(different_timestamp_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different timestamp_ms should NOT be deduplicated"
    );

    // Test Case 8: Different access_type field should NOT be deduplicated
    let different_access_type_peers =
        vec![signed_peer_base.clone(), signed_peer_different_access_type];
    let result = deduplicate_peers(different_access_type_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with different access_type should NOT be deduplicated"
    );

    // Test Case 9: Different signature (same data, different key) should NOT be
    // deduplicated
    let different_signature_peers = vec![signed_peer_base.clone(), signed_peer_base_different_key];
    let result = deduplicate_peers(different_signature_peers);
    assert_eq!(
        result.len(),
        2,
        "Peers with same data but different signatures should NOT be deduplicated"
    );

    // Test Case 10: Edge cases
    // Empty input should return empty output
    let empty_result = deduplicate_peers(vec![]);
    assert_eq!(
        empty_result.len(),
        0,
        "Empty input should return empty output"
    );

    // Single peer should return single peer
    let single_input = vec![signed_peer_base.clone()];
    let single_result = deduplicate_peers(single_input);
    assert_eq!(
        single_result.len(),
        1,
        "Single input should return single output"
    );

    // Multiple identical peers should return single peer
    let multiple_identical = vec![
        signed_peer_base.clone(),
        signed_peer_base.clone(),
        signed_peer_base.clone(),
    ];
    let multiple_identical_result = deduplicate_peers(multiple_identical);
    assert_eq!(
        multiple_identical_result.len(),
        1,
        "Multiple identical peers should be deduplicated to 1 peer"
    );

    // Test Case 11: Mixed scenario with multiple duplicates and unique peers
    let mixed_peers = vec![
        signed_peer_base.clone(),                // Original
        signed_peer_base,                        // Duplicate of original
        signed_peer_different_timestamp.clone(), // Different (timestamp)
        signed_peer_different_timestamp,         // Duplicate of different
        signed_peer_other,                       // Completely unique
    ];
    let mixed_result = deduplicate_peers(mixed_peers);
    assert_eq!(
        mixed_result.len(),
        3,
        "Mixed duplicates and unique peers should result in 3 unique entries"
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_private_address_filtering() -> Result<()> {
    // Test that private and unroutable addresses are filtered out during peer
    // discovery

    // Create a test network and state
    let (discovery, _server, network, key) = set_up_network(P2pConfig::default(), None);

    // Set up the nodes's own info to avoid unwrap panics
    let signed_peer_info = NodeInfo {
        peer_id: network.peer_id(),
        addresses: Vec::new(),
        timestamp_ms: now_unix(),
        access_type: AccessType::Public,
    }
    .sign(&key);

    let (_event_loop, _handle, state) = start_network(discovery, network.clone(), key);
    state.write().unwrap().our_info = Some(signed_peer_info);

    // Define test cases with various private/unroutable addresses that should be
    // filtered
    let filtered_addresses = vec![
        // IPv4 private networks (RFC 1918)
        "/ip4/192.168.1.100/udp/12345",
        "/ip4/10.0.0.1/udp/12345",
        "/ip4/172.16.0.1/udp/12345",
        // IPv4 loopback
        "/ip4/127.0.0.1/udp/12345",
        // IPv4 link-local (RFC 3927)
        "/ip4/169.254.1.1/udp/12345",
        // IPv4 multicast
        "/ip4/224.0.0.1/udp/12345",
        // IPv4 carrier-grade NAT (RFC 6598)
        "/ip4/100.64.0.1/udp/12345",
        // IPv4 documentation addresses (RFC 5737)
        "/ip4/192.0.2.1/udp/12345",
        "/ip4/198.51.100.1/udp/12345",
        "/ip4/203.0.113.1/udp/12345",
        // IPv6 loopback
        "/ip6/::1/udp/12345",
        // IPv6 unique local addresses (RFC 4193)
        "/ip6/fc00::1/udp/12345",
        "/ip6/fd00::1/udp/12345",
        // IPv6 link-local (RFC 4862)
        "/ip6/fe80::1/udp/12345",
        // IPv6 documentation (RFC 3849)
        "/ip6/2001:db8::1/udp/12345",
        // IPv6 multicast
        "/ip6/ff02::1/udp/12345",
    ];

    // Test public addresses that should NOT be filtered (but may fail verification
    // due to unreachability)
    let public_addresses = [
        // Valid public IPv4 addresses using invalid domains for testing
        "/dns/nonexistent-test-domain-for-iota.invalid/udp/12345",
        "/dns/test-peer-unreachable.invalid/udp/12345",
    ];

    let mut filtered_peer_infos = Vec::new();
    let mut filtered_peer_ids = Vec::new();

    // Create peers with private/unroutable addresses (should be filtered)
    for (i, address_str) in filtered_addresses.iter().enumerate() {
        let key = NetworkKeyPair::generate(&mut rand::thread_rng());
        let peer_id = anemo::PeerId(key.public().0.to_bytes());

        let peer_info = NodeInfo {
            peer_id,
            addresses: vec![address_str.parse().unwrap()],
            timestamp_ms: now_unix() + i as u64,
            access_type: AccessType::Public,
        };
        let signed_peer_info = peer_info.sign(&key);

        filtered_peer_infos.push(signed_peer_info);
        filtered_peer_ids.push(peer_id);
    }

    let mut public_peer_infos = Vec::new();
    let mut public_peer_ids = Vec::new();

    // Create peers with public addresses (should reach verification)
    for (i, address_str) in public_addresses.iter().enumerate() {
        let key = NetworkKeyPair::generate(&mut rand::thread_rng());
        let peer_id = anemo::PeerId(key.public().0.to_bytes());

        let peer_info = NodeInfo {
            peer_id,
            addresses: vec![address_str.parse().unwrap()],
            timestamp_ms: now_unix() + 1000 + i as u64,
            access_type: AccessType::Public,
        };
        let signed_peer_info = peer_info.sign(&key);

        public_peer_infos.push(signed_peer_info);
        public_peer_ids.push(peer_id);
    }

    // Combine all peer infos and update
    let mut all_peer_infos = filtered_peer_infos;
    all_peer_infos.extend(public_peer_infos);

    update_peers_for_test(&network, state.clone(), all_peer_infos, false).await;

    let state_guard = state.read().unwrap();

    // Verify that all private/unroutable address peers are filtered out before
    // verification
    for (i, peer_id) in filtered_peer_ids.iter().enumerate() {
        assert!(
            !state_guard.known_peers.contains_key(peer_id),
            "Peer {} with private/unroutable address {} should be filtered out",
            i,
            filtered_addresses[i]
        );
        assert!(
            !state_guard
                .address_verification_cooldown
                .contains_key(peer_id),
            "Peer {} with private/unroutable address {} should not even reach verification cooldown",
            i,
            filtered_addresses[i]
        );
    }

    // Verify that public address peers reach verification (even if they fail due to
    // unreachability) They either get added to known_peers or added to
    // verification cooldown
    for (i, peer_id) in public_peer_ids.iter().enumerate() {
        let public_peer_processed = state_guard.known_peers.contains_key(peer_id)
            || state_guard
                .address_verification_cooldown
                .contains_key(peer_id);
        assert!(
            public_peer_processed,
            "Peer {} with public address {} should at least reach verification step",
            i, public_addresses[i]
        );
    }

    Ok(())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_construct_our_info_address_filtering() -> Result<()> {
    // Test that private addresses are filtered out and valid public addresses are
    // included during our own info construction

    // Test cases: (address, should_be_included, description)
    let test_addresses = vec![
        // Private/unroutable addresses - should be filtered out
        (
            "/ip4/192.168.1.100/udp/12345",
            false,
            "IPv4 private network (RFC 1918)",
        ),
        (
            "/ip4/10.0.0.1/udp/12345",
            false,
            "IPv4 private network (RFC 1918)",
        ),
        (
            "/ip4/172.16.0.1/udp/12345",
            false,
            "IPv4 private network (RFC 1918)",
        ),
        ("/ip4/127.0.0.1/udp/12345", false, "IPv4 loopback"),
        (
            "/ip4/169.254.1.1/udp/12345",
            false,
            "IPv4 link-local (RFC 3927)",
        ),
        ("/ip4/224.0.0.1/udp/12345", false, "IPv4 multicast"),
        (
            "/ip4/100.64.0.1/udp/12345",
            false,
            "IPv4 carrier-grade NAT (RFC 6598)",
        ),
        ("/ip6/::1/udp/12345", false, "IPv6 loopback"),
        (
            "/ip6/fc00::1/udp/12345",
            false,
            "IPv6 unique local (RFC 4193)",
        ),
        (
            "/ip6/fe80::1/udp/12345",
            false,
            "IPv6 link-local (RFC 4862)",
        ),
        (
            "/ip6/2001:db8::1/udp/12345",
            false,
            "IPv6 documentation (RFC 3849)",
        ),
        // Unsupported address formats - should be filtered out
        (
            "/dns4/iota.org/udp/12345",
            false,
            "DNS4 hostname (unsupported by anemo)",
        ),
        (
            "/dns6/iota.org/udp/12345",
            false,
            "DNS6 hostname (unsupported by anemo)",
        ),
        // Invalid DNS addresses - should be filtered out
        (
            "/dns/localhost/udp/12345",
            false,
            "localhost hostname (invalid FQDN)",
        ),
        (
            "/dns/test.local/udp/12345",
            false,
            ".local domain (invalid FQDN)",
        ),
        (
            "/dns/hostname/udp/12345",
            false,
            "single label hostname (invalid FQDN)",
        ),
        // Valid public addresses - should be included
        ("/ip4/8.8.8.8/udp/12345", true, "Google DNS IPv4"),
        ("/ip4/1.1.1.1/udp/12345", true, "Cloudflare DNS IPv4"),
        (
            "/ip6/2001:4860:4860::8888/udp/12345",
            true,
            "Google DNS IPv6",
        ),
        (
            "/ip6/2606:4700:4700::1111/udp/12345",
            true,
            "Cloudflare DNS IPv6",
        ),
        ("/dns/example.com/udp/12345", true, "DNS hostname"),
    ];

    for (address_str, should_be_included, description) in test_addresses {
        // Create a config with the test external address
        let config = P2pConfig {
            external_address: Some(address_str.parse().unwrap()),
            ..Default::default()
        };

        let (discovery, _server, network, key) = set_up_network(config, None);
        let (event_loop, _handle, state) =
            start_network_without_external_address(discovery, network.clone(), key);

        // Start the event loop which will call construct_our_info
        tokio::spawn(event_loop.start());

        // Give it a moment to initialize
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let state_guard = state.read().unwrap();
        let our_info = state_guard.our_info.as_ref().unwrap();

        if should_be_included {
            // Valid public addresses should be included
            assert!(
                !our_info.addresses.is_empty(),
                "Valid public address {address_str} ({description}) should be included in our own info"
            );

            // Verify the address is actually included
            let address_found = our_info
                .addresses
                .iter()
                .any(|addr| addr.to_string() == address_str);
            assert!(
                address_found,
                "Valid public address {address_str} ({description}) should be found in our addresses: {:?}",
                our_info.addresses
            );
        } else {
            // Private/unroutable addresses should be filtered out
            assert!(
                our_info.addresses.is_empty(),
                "Private/unroutable address {address_str} ({description}) should be filtered out from our own info",
            );
        }
    }

    Ok(())
}
