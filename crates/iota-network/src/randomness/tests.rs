// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use anemo::types::response::StatusCode;
use fastcrypto::{
    groups::{GroupElement, bls12381},
    serde_helpers::ToFromByteArray,
};
use fastcrypto_threshold::{mocked_dkg, nodes};
use iota_macros::sim_test;
use iota_swarm_config::test_utils::CommitteeFixture;
use iota_types::{
    base_types::ConciseableName,
    committee::Committee,
    crypto::{AuthorityPublicKeyBytes, ToFromBytes},
};
use tracing::Instrument;

use crate::{randomness::*, utils};

type PkG = bls12381::G2Element;
type EncG = bls12381::G2Element;

#[sim_test]
async fn test_multiple_epochs() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (authority, stake) in committee.members() {
        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).build();

        let network = utils::build_network(|r| r.merge(router));
        for n in networks.iter() {
            network.connect(n.local_addr()).await.unwrap();
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));

        let span = tracing::span!(
            tracing::Level::INFO,
            "RandomnessEventLoop",
            authority = ?authority.concise(),
        );
        tokio::spawn(r.start().instrument(span));
    }
    info!(?authority_info, "authorities constructed");

    let nodes = nodes::Nodes::new(nodes).unwrap();

    // Test first round.
    for (authority, handle) in handles.iter() {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            0,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(0, RandomnessRound::new(0));
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    for rx in randomness_rxs.iter_mut() {
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(0, epoch);
        assert_eq!(0, round.value());
        assert_ne!(0, bytes.len());
    }

    // Test a few more rounds. Generation of rounds in epoch 1 should block until
    // epoch is updated.
    for (_authority, handle) in handles.iter() {
        handle.complete_round(0, RandomnessRound::new(0));
        handle.send_partial_signatures(0, RandomnessRound::new(1));
        handle.send_partial_signatures(1, RandomnessRound::new(0));
        handle.send_partial_signatures(1, RandomnessRound::new(1));
    }
    for rx in randomness_rxs.iter_mut() {
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(0, epoch);
        assert_eq!(1, round.value());
        assert_ne!(0, bytes.len());
        assert!(rx.try_recv().is_err()); // there should not be anything else
        // ready yet
    }
    for (authority, handle) in handles.iter() {
        // update to epoch 1
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            1,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.update_epoch(
            1,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    let mut rounds_seen = BTreeSet::new(); // use a set because rounds could be generated out-of-order
    for rx in randomness_rxs.iter_mut() {
        // now we expect the two rounds we started earlier to be generated
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(1, epoch);
        rounds_seen.insert(round);
        assert_ne!(0, bytes.len());
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(1, epoch);
        rounds_seen.insert(round);
        assert_ne!(0, bytes.len());
    }
    assert!(rounds_seen.contains(&RandomnessRound::new(0)));
    assert!(rounds_seen.contains(&RandomnessRound::new(1)));
}

#[sim_test]
async fn test_record_own_partial_sigs() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (authority, stake) in committee.members() {
        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).build();

        let network = utils::build_network(|r| r.merge(router));
        for n in networks.iter() {
            network.connect(n.local_addr()).await.unwrap();
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));

        let span = tracing::span!(
            tracing::Level::INFO,
            "RandomnessEventLoop",
            authority = ?authority.concise(),
        );
        tokio::spawn(r.start().instrument(span));
    }
    info!(?authority_info, "authorities constructed");

    let nodes = nodes::Nodes::new(nodes).unwrap();

    // Only send partial sigs from authorities 0 and 1. They should still be able to
    // reach the threshold to generate full signatures, only if they are
    // correctly recording and using their own partial signatures as well.
    for (authority, handle) in handles.iter().take(2) {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            0,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(0, RandomnessRound::new(0));
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    for (i, rx) in randomness_rxs.iter_mut().enumerate() {
        if i < 2 {
            let (epoch, round, bytes) = rx.recv().await.unwrap();
            assert_eq!(0, epoch);
            assert_eq!(0, round.value());
            assert_ne!(0, bytes.len());
        } else {
            assert!(rx.try_recv().is_err());
        }
    }
}

#[sim_test]
async fn test_receive_full_sig() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 8);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (i, (authority, stake)) in committee.members().enumerate() {
        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).build();

        let network = utils::build_network(|r| r.merge(router));
        if i < 7 {
            // Leave last network disconnected to test full sig distribution.
            for n in networks.iter() {
                network.connect(n.local_addr()).await.unwrap();
            }
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));

        let span = tracing::span!(
            tracing::Level::INFO,
            "RandomnessEventLoop",
            authority = ?authority.concise(),
        );
        tokio::spawn(r.start().instrument(span));
    }
    info!(?authority_info, "authorities constructed");

    let nodes = nodes::Nodes::new(nodes).unwrap();

    // Authorities 0-6 should be able to complete the sig as normal
    // using partial sigs.
    for (authority, handle) in handles.iter() {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            0,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(0, RandomnessRound::new(0));
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    for rx in randomness_rxs[..7].iter_mut() {
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(0, epoch);
        assert_eq!(0, round.value());
        assert_ne!(0, bytes.len());
    }
    // Authority 7 shouldn't have the completed sig, since it's disconnected.
    assert!(randomness_rxs[7].try_recv().is_err());

    // Connect authority 7 to a single other authority, which should be able to
    // transmit the full sig.
    networks[7].connect(networks[0].local_addr()).await.unwrap();
    let (epoch, round, bytes) = randomness_rxs[7].recv().await.unwrap();
    assert_eq!(0, epoch);
    assert_eq!(0, round.value());
    assert_ne!(0, bytes.len());
}

#[sim_test]
async fn test_restart_recovery() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (authority, stake) in committee.members() {
        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).build();

        let network = utils::build_network(|r| r.merge(router));
        for n in networks.iter() {
            network.connect(n.local_addr()).await.unwrap();
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));

        let span = tracing::span!(
            tracing::Level::INFO,
            "RandomnessEventLoop",
            authority = ?authority.concise(),
        );
        tokio::spawn(r.start().instrument(span));
    }
    info!(?authority_info, "authorities constructed");

    let nodes = nodes::Nodes::new(nodes).unwrap();

    // Test first round.
    for (authority, handle) in handles.iter() {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            0,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(0, RandomnessRound::new(1_000_000));
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            Some(RandomnessRound::new(999_999)),
        );
    }
    for rx in randomness_rxs.iter_mut() {
        let (epoch, round, bytes) = rx.recv().await.unwrap();
        assert_eq!(0, epoch);
        assert_eq!(1_000_000, round.value());
        assert_ne!(0, bytes.len());
    }
}

#[sim_test]
async fn test_byzantine_peer_handling() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (authority, stake) in committee.members() {
        let config = RandomnessConfig {
            max_ignored_peer_weight_factor: Some(0.3),
            ..Default::default()
        };

        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).config(config).build();

        let network = utils::build_network(|r| r.merge(router));
        for n in networks.iter() {
            network.connect(n.local_addr()).await.unwrap();
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));

        let span = tracing::span!(
            tracing::Level::INFO,
            "RandomnessEventLoop",
            authority = ?authority.concise(),
        );
        tokio::spawn(r.start().instrument(span));
    }
    info!(?authority_info, "authorities constructed");

    let nodes = nodes::Nodes::new(nodes).unwrap();

    // Send partial sigs from authorities 0 and 1, but give them different DKG
    // output so they think each other are byzantine.
    for (i, (authority, handle)) in handles.iter().enumerate() {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            if i < 2 { 100 + i as u128 } else { 0 },
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(0, RandomnessRound::new(0));
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }

    for rx in &mut randomness_rxs[2..] {
        // Validators (2, 3) can communicate normally.
        let (epoch, round, bytes) = rx
            .recv()
            .await
            .expect("Validators (2, 3) should receive randomness in epoch 0, round 0");
        assert_eq!(0, epoch);
        assert_eq!(0, round.value());
        assert_ne!(0, bytes.len());
    }
    for rx in &mut randomness_rxs[..2] {
        // Validators (0, 1) are byzantine.
        assert!(rx.try_recv().is_err());
    }

    // New epoch, they should forgive each other.
    for (authority, handle) in handles.iter().take(2) {
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            0,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        handle.send_partial_signatures(1, RandomnessRound::new(0));
        handle.update_epoch(
            1,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    for rx in &mut randomness_rxs[..2] {
        // Validators (0, 1) can communicate normally in new epoch.
        let (epoch, round, bytes) = rx
            .recv()
            .await
            .expect("Validators (0, 1) should receive randomness in epoch 1, round 0");
        assert_eq!(1, epoch);
        assert_eq!(0, round.value());
        assert_ne!(0, bytes.len());
    }
    for rx in &mut randomness_rxs[2..] {
        // Validators (2, 3) are still on old epoch.
        assert!(rx.try_recv().is_err());
    }
}

#[sim_test]
async fn test_full_sig_from_checkpoint() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();

    let mut randomness_rxs = Vec::new();
    let mut networks: Vec<anemo::Network> = Vec::new();
    let mut nodes = Vec::new();
    let mut handles = Vec::new();
    let mut authority_info = HashMap::new();

    for (i, (authority, stake)) in committee.members().enumerate() {
        let (tx, rx) = mpsc::channel(3);
        randomness_rxs.push(rx);
        let (unstarted, router) = Builder::new(*authority, tx).build();

        let network = utils::build_network(|r| r.merge(router));
        // Authority 3 stays disconnected, so it can only complete a round from
        // a signature handed to it directly.
        if i < 3 {
            for n in networks.iter() {
                network.connect(n.local_addr()).await.unwrap();
            }
        }
        networks.push(network.clone());

        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (network.peer_id(), node.id));
        nodes.push(node);

        let (r, handle) = unstarted.build(network);
        handles.push((authority, handle));
        tokio::spawn(r.start());
    }
    let nodes = nodes::Nodes::new(nodes).unwrap();

    for (i, (authority, handle)) in handles.iter().enumerate() {
        // A non-zero full private key: with 0 every signature is the identity
        // element, which verifies for any round.
        let mock_dkg_output = mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            committee.validity_threshold().try_into().unwrap(),
            1,
            committee
                .authority_index(authority)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        if i < 3 {
            handle.send_partial_signatures(0, RandomnessRound::new(0));
        }
        handle.update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output,
            committee.validity_threshold().try_into().unwrap(),
            None,
        );
    }
    let (_, _, round_0_bytes) = randomness_rxs[0].recv().await.unwrap();
    let round_0_sig: RandomnessSignature = bcs::from_bytes(&round_0_bytes).unwrap();

    let handle_3 = &handles[3].1;
    // A valid signature for round 0, and the same signature claimed for round
    // 1, where it does not verify. Both arrive before local consensus
    // requests either round.
    handle_3.receive_full_signature_from_checkpoint(0, RandomnessRound::new(0), round_0_sig);
    handle_3.receive_full_signature_from_checkpoint(0, RandomnessRound::new(1), round_0_sig);
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(randomness_rxs[3].try_recv().is_err());

    handle_3.send_partial_signatures(0, RandomnessRound::new(0));
    let (epoch, round, bytes) = randomness_rxs[3].recv().await.unwrap();
    assert_eq!(0, epoch);
    assert_eq!(0, round.value());
    assert_eq!(round_0_bytes, bytes);

    handle_3.send_partial_signatures(0, RandomnessRound::new(1));
    tokio::time::sleep(Duration::from_secs(5)).await;
    let unexpected = randomness_rxs[3].try_recv();
    assert!(unexpected.is_err(), "unexpected randomness: {unexpected:?}");
}

#[tokio::test]
async fn test_full_peer_mailbox_rejects_requests() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();
    let (authority, _) = committee.members().next().unwrap();

    let config = RandomnessConfig {
        mailbox_capacity: Some(2),
        peer_mailbox_capacity: Some(2),
        ..Default::default()
    };
    let (tx, _rx) = mpsc::channel(3);
    let (unstarted, router) = Builder::new(*authority, tx).config(config).build();
    let network = utils::build_network(|r| r.merge(router));
    let peer_network = utils::build_network(|r| r);
    peer_network.connect(network.local_addr()).await.unwrap();

    // The event loop is not started yet, so nothing drains its mailboxes.
    let (event_loop, handle) = unstarted.build(network.clone());
    event_loop
        .allowed_peers
        .update(Arc::new(HashSet::from([peer_network.peer_id()])));

    let mut client = RandomnessClient::new(peer_network.peer(network.peer_id()).unwrap());
    let request = || {
        anemo::Request::new(SendSignaturesRequest {
            epoch: 0,
            round: RandomnessRound::new(0),
            partial_sigs: Vec::new(),
            sig: None,
        })
        .with_timeout(Duration::from_secs(5))
    };
    for _ in 0..2 {
        client.send_signatures(request()).await.unwrap();
    }
    let status = client.send_signatures(request()).await.unwrap_err();
    assert_eq!(status.status(), StatusCode::TooManyRequests);

    // A full peer mailbox leaves room for the node's own messages.
    handle.send_partial_signatures(0, RandomnessRound::new(0));
    handle.complete_round(0, RandomnessRound::new(0));

    // Once the event loop drains the mailbox, requests are accepted again.
    tokio::spawn(event_loop.start());
    tokio::time::timeout(Duration::from_secs(10), async {
        while client.send_signatures(request()).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("request should be accepted once the mailbox drains");
}

#[tokio::test]
async fn test_queued_requests_per_peer_are_limited() {
    telemetry_subscribers::init_for_testing();
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();
    let (authority, _) = committee.members().next().unwrap();

    let config = RandomnessConfig {
        max_queued_requests_per_peer: Some(2),
        ..Default::default()
    };
    let (tx, _rx) = mpsc::channel(3);
    let (unstarted, router) = Builder::new(*authority, tx).config(config).build();
    let network = utils::build_network(|r| r.merge(router));
    let flooding_network = utils::build_network(|r| r);
    let honest_network = utils::build_network(|r| r);
    flooding_network
        .connect(network.local_addr())
        .await
        .unwrap();
    honest_network.connect(network.local_addr()).await.unwrap();

    // The event loop is not started yet, so nothing drains its mailboxes.
    let (event_loop, _handle) = unstarted.build(network.clone());
    event_loop.allowed_peers.update(Arc::new(HashSet::from([
        flooding_network.peer_id(),
        honest_network.peer_id(),
    ])));

    let mut flooding_client =
        RandomnessClient::new(flooding_network.peer(network.peer_id()).unwrap());
    let mut honest_client = RandomnessClient::new(honest_network.peer(network.peer_id()).unwrap());
    let request = || {
        anemo::Request::new(SendSignaturesRequest {
            epoch: 0,
            round: RandomnessRound::new(0),
            partial_sigs: Vec::new(),
            sig: None,
        })
        .with_timeout(Duration::from_secs(5))
    };
    for _ in 0..2 {
        flooding_client.send_signatures(request()).await.unwrap();
    }
    let status = flooding_client
        .send_signatures(request())
        .await
        .unwrap_err();
    assert_eq!(status.status(), StatusCode::TooManyRequests);

    // Another peer's requests are still accepted.
    honest_client.send_signatures(request()).await.unwrap();

    // Once the event loop handles the queued requests, the peer may send again.
    tokio::spawn(event_loop.start());
    tokio::time::timeout(Duration::from_secs(10), async {
        while flooding_client.send_signatures(request()).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("request should be accepted once the queued requests are handled");
}

/// Builds the event loop of the first member of a 4-member committee, in
/// epoch 0 with round 0 requested, and returns it with the mocked DKG output of
/// the second member and that member's peer ID.
fn event_loop_at_round_0() -> (RandomnessEventLoop, dkg_v1::Output<PkG, EncG>, PeerId) {
    let committee_fixture =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    let committee = committee_fixture.committee();
    let config = RandomnessConfig {
        max_ignored_peer_weight_factor: Some(0.3),
        ..Default::default()
    };

    let mut authority_info = HashMap::new();
    let mut nodes = Vec::new();
    for (i, (authority, stake)) in committee.members().enumerate() {
        let node = node_from_committee(committee, authority, *stake);
        authority_info.insert(*authority, (PeerId([i as u8; 32]), node.id));
        nodes.push(node);
    }
    let nodes = nodes::Nodes::new(nodes).unwrap();
    let threshold = committee.validity_threshold();
    let mock_dkg_output = |party_id| {
        mocked_dkg::generate_mocked_output::<PkG, EncG>(
            nodes.clone(),
            threshold.try_into().unwrap(),
            0,
            party_id,
        )
    };

    let (authority, _) = committee.members().next().unwrap();
    let (tx, _rx) = mpsc::channel(3);
    let (unstarted, _router) = Builder::new(*authority, tx).config(config).build();
    let (mut event_loop, _handle) = unstarted.build(utils::build_network(|r| r));
    let own_party_id = authority_info[authority].1;
    event_loop
        .update_epoch(
            0,
            authority_info.clone(),
            mock_dkg_output(own_party_id),
            threshold.try_into().unwrap(),
            None,
        )
        .unwrap();
    event_loop.send_partial_signatures(0, RandomnessRound::new(0));

    let (peer_name, _) = committee.members().nth(1).unwrap();
    let (peer_id, peer_party_id) = authority_info[peer_name];
    (event_loop, mock_dkg_output(peer_party_id), peer_id)
}

#[tokio::test]
async fn test_invalid_full_sig_ignores_peer() {
    let (mut event_loop, _, peer_id) = event_loop_at_round_0();
    assert!(event_loop.allowed_peers_set.contains(&peer_id));

    event_loop.receive_full_signature(
        peer_id,
        0,
        RandomnessRound::new(0),
        RandomnessSignature::generator(),
    );

    assert!(!event_loop.allowed_peers_set.contains(&peer_id));
}

#[tokio::test]
async fn test_invalid_partial_sigs_ignore_peer() {
    let (mut event_loop, peer_dkg_output, peer_id) = event_loop_at_round_0();
    assert!(event_loop.allowed_peers_set.contains(&peer_id));

    // Signed for the wrong round, so they fail verification for round 0.
    let partial_sigs = ThresholdBls12381MinSig::partial_sign_batch(
        peer_dkg_output.shares.unwrap().iter(),
        &RandomnessRound::new(1).signature_message(),
    );
    let sig_bytes = partial_sigs
        .iter()
        .map(|sig| bcs::to_bytes(sig).unwrap())
        .collect();
    event_loop.receive_partial_signatures(peer_id, 0, RandomnessRound::new(0), sig_bytes);

    assert!(!event_loop.allowed_peers_set.contains(&peer_id));
}

fn node_from_committee(
    committee: &Committee,
    authority: &AuthorityPublicKeyBytes,
    stake: u64,
) -> nodes::Node<EncG> {
    let id = committee
        .authority_index(authority)
        .unwrap()
        .try_into()
        .unwrap();
    let pk = bls12381::G2Element::from_byte_array(
        committee
            .public_key(authority)
            .expect("lookup of known committee member should succeed")
            .as_bytes()
            .try_into()
            .expect("key length should match"),
    )
    .expect("should work to convert BLS key to G2Element");
    fastcrypto_threshold::nodes::Node::<EncG> {
        id,
        pk: fastcrypto_threshold::ecies_v1::PublicKey::from(pk),
        weight: stake.try_into().unwrap(),
    }
}
