// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc, time::Duration};

use anemo::{PeerId, types::PeerEvent};
use dashmap::DashMap;
use futures::future;
use iota_metrics::{metrics_network::NetworkConnectionMetrics, spawn_logged_monitored_task};
use quinn_proto::ConnectionStats;
use tokio::{sync::broadcast, task::JoinHandle, time};

const CONNECTION_STAT_COLLECTION_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct ConditionalBroadcastReceiver {
    pub receiver: broadcast::Receiver<()>,
}

/// ConditionalBroadcastReceiver has an additional method for convenience to be
/// able to use to conditionally check for shutdown in all branches of a select
/// statement. Using this method will allow for the shutdown signal to propagate
/// faster, since we will no longer be waiting until the branch that checks the
/// receiver is randomly selected by the select macro.
impl ConditionalBroadcastReceiver {
    pub async fn received_signal(&mut self) -> bool {
        futures::future::poll_immediate(&mut Box::pin(self.receiver.recv()))
            .await
            .is_some()
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
}

pub struct ConnectionMonitor {
    network: anemo::NetworkRef,
    connection_metrics: NetworkConnectionMetrics,
    peer_id_types: HashMap<PeerId, String>,
    connection_statuses: Arc<DashMap<PeerId, ConnectionStatus>>,
    rx_shutdown: Option<ConditionalBroadcastReceiver>,
}

impl ConnectionMonitor {
    #[must_use]
    pub fn spawn(
        network: anemo::NetworkRef,
        connection_metrics: NetworkConnectionMetrics,
        peer_id_types: HashMap<PeerId, String>,
        rx_shutdown: Option<ConditionalBroadcastReceiver>,
    ) -> (JoinHandle<()>, Arc<DashMap<PeerId, ConnectionStatus>>) {
        let connection_statuses_outer = Arc::new(DashMap::new());
        let connection_statuses = connection_statuses_outer.clone();
        (
            spawn_logged_monitored_task!(
                Self {
                    network,
                    connection_metrics,
                    peer_id_types,
                    connection_statuses,
                    rx_shutdown
                }
                .run(),
                "ConnectionMonitor"
            ),
            connection_statuses_outer,
        )
    }

    async fn run(mut self) {
        let (mut subscriber, connected_peers) = {
            if let Some(network) = self.network.upgrade() {
                let Ok((subscriber, active_peers)) = network.subscribe() else {
                    return;
                };
                (subscriber, active_peers)
            } else {
                return;
            }
        };

        // we report first all the known peers as disconnected - so we can see
        // their labels in the metrics reporting tool
        let mut known_peers = Vec::new();
        for (peer_id, ty) in &self.peer_id_types {
            known_peers.push(*peer_id);
            self.connection_metrics
                .network_peer_connected
                .with_label_values(&[&format!("{peer_id}"), ty])
                .set(0)
        }

        // now report the connected peers
        for peer_id in connected_peers.iter() {
            self.handle_peer_event(PeerEvent::NewPeer(*peer_id)).await;
        }

        let mut connection_stat_collection_interval =
            time::interval(CONNECTION_STAT_COLLECTION_INTERVAL);

        async fn wait_for_shutdown(
            rx_shutdown: &mut Option<ConditionalBroadcastReceiver>,
        ) -> Result<(), tokio::sync::broadcast::error::RecvError> {
            if let Some(rx) = rx_shutdown.as_mut() {
                rx.receiver.recv().await
            } else {
                // If no shutdown receiver is provided, wait forever.
                let future = future::pending();
                #[allow(clippy::let_unit_value)]
                let () = future.await;
                Ok(())
            }
        }

        loop {
            tokio::select! {
                _ = connection_stat_collection_interval.tick() => {
                    if let Some(network) = self.network.upgrade() {
                        self.connection_metrics.socket_receive_buffer_size.set(
                            network.socket_receive_buf_size() as i64
                        );
                        self.connection_metrics.socket_send_buffer_size.set(
                            network.socket_send_buf_size() as i64
                        );
                        for peer_id in known_peers.iter() {
                            if let Some(connection) = network.peer(*peer_id) {
                                let stats = connection.connection_stats();
                                self.update_quinn_metrics_for_peer(&format!("{peer_id}"), &stats);
                            }
                        }
                    } else {
                        continue;
                    }
                }
                Ok(event) = subscriber.recv() => {
                    self.handle_peer_event(event).await;
                }
                _ = wait_for_shutdown(&mut self.rx_shutdown) => {
                    return;
                }
            }
        }
    }

    async fn handle_peer_event(&self, peer_event: PeerEvent) {
        if let Some(network) = self.network.upgrade() {
            self.connection_metrics
                .network_peers
                .set(network.peers().len() as i64);
        } else {
            return;
        }

        let (peer_id, status, int_status) = match peer_event {
            PeerEvent::NewPeer(peer_id) => (peer_id, ConnectionStatus::Connected, 1),
            PeerEvent::LostPeer(peer_id, _) => (peer_id, ConnectionStatus::Disconnected, 0),
        };
        self.connection_statuses.insert(peer_id, status);

        // Only report peer IDs for known peers to prevent unlimited cardinality.
        let peer_id_str = if self.peer_id_types.contains_key(&peer_id) {
            format!("{peer_id}")
        } else {
            "other_peer".to_string()
        };

        if let Some(ty) = self.peer_id_types.get(&peer_id) {
            self.connection_metrics
                .network_peer_connected
                .with_label_values(&[&peer_id_str, ty])
                .set(int_status);
        }

        if let PeerEvent::LostPeer(_, reason) = peer_event {
            self.connection_metrics
                .network_peer_disconnects
                .with_label_values(&[&peer_id_str, &format!("{reason:?}")])
                .inc();
        }
    }

    // TODO: Replace this with ClosureMetric
    fn update_quinn_metrics_for_peer(&self, peer_id: &str, stats: &ConnectionStats) {
        // Update PathStats
        self.connection_metrics
            .network_peer_rtt
            .with_label_values(&[peer_id])
            .set(stats.path.rtt.as_millis() as i64);
        self.connection_metrics
            .network_peer_lost_packets
            .with_label_values(&[peer_id])
            .set(stats.path.lost_packets as i64);
        self.connection_metrics
            .network_peer_lost_bytes
            .with_label_values(&[peer_id])
            .set(stats.path.lost_bytes as i64);
        self.connection_metrics
            .network_peer_sent_packets
            .with_label_values(&[peer_id])
            .set(stats.path.sent_packets as i64);
        self.connection_metrics
            .network_peer_congestion_events
            .with_label_values(&[peer_id])
            .set(stats.path.congestion_events as i64);
        self.connection_metrics
            .network_peer_congestion_window
            .with_label_values(&[peer_id])
            .set(stats.path.cwnd as i64);

        // Update FrameStats
        self.connection_metrics
            .network_peer_max_data
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.frame_tx.max_data as i64);
        self.connection_metrics
            .network_peer_max_data
            .with_label_values(&[peer_id, "received"])
            .set(stats.frame_rx.max_data as i64);
        self.connection_metrics
            .network_peer_closed_connections
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.frame_tx.connection_close as i64);
        self.connection_metrics
            .network_peer_closed_connections
            .with_label_values(&[peer_id, "received"])
            .set(stats.frame_rx.connection_close as i64);
        self.connection_metrics
            .network_peer_data_blocked
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.frame_tx.data_blocked as i64);
        self.connection_metrics
            .network_peer_data_blocked
            .with_label_values(&[peer_id, "received"])
            .set(stats.frame_rx.data_blocked as i64);

        // Update UDPStats
        self.connection_metrics
            .network_peer_udp_datagrams
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.udp_tx.datagrams as i64);
        self.connection_metrics
            .network_peer_udp_datagrams
            .with_label_values(&[peer_id, "received"])
            .set(stats.udp_rx.datagrams as i64);
        self.connection_metrics
            .network_peer_udp_bytes
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.udp_tx.bytes as i64);
        self.connection_metrics
            .network_peer_udp_bytes
            .with_label_values(&[peer_id, "received"])
            .set(stats.udp_rx.bytes as i64);
        self.connection_metrics
            .network_peer_udp_transmits
            .with_label_values(&[peer_id, "transmitted"])
            .set(stats.udp_tx.ios as i64);
        self.connection_metrics
            .network_peer_udp_transmits
            .with_label_values(&[peer_id, "received"])
            .set(stats.udp_rx.ios as i64);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, convert::Infallible, time::Duration};

    use anemo::{Network, Request, Response};
    use bytes::Bytes;
    use iota_metrics::metrics_network::NetworkConnectionMetrics;
    use prometheus::Registry;
    use tokio::{
        sync::{broadcast, broadcast::error::SendError},
        time::{sleep, timeout},
    };
    use tower::util::BoxCloneService;

    use crate::connection_monitor::{
        ConditionalBroadcastReceiver, ConnectionMonitor, ConnectionStatus,
    };

    /// PreSubscribedBroadcastSender is a wrapped Broadcast channel that limits
    /// subscription to initialization time. This is designed to be used for
    /// cancellation signal to all the components, and the limitation is
    /// intended to prevent a component missing the shutdown signal due to a
    /// subscription that happens after the shutdown signal was sent. The
    /// receivers have a special peek method which can be used to
    /// conditionally check for shutdown signal on the channel.
    struct PreSubscribedBroadcastSender {
        sender: broadcast::Sender<()>,
        receivers: Vec<ConditionalBroadcastReceiver>,
    }

    impl PreSubscribedBroadcastSender {
        fn new(num_subscribers: u64) -> Self {
            let (tx_init, _) = broadcast::channel(1);
            let mut receivers = Vec::new();
            for _i in 0..num_subscribers {
                receivers.push(ConditionalBroadcastReceiver {
                    receiver: tx_init.subscribe(),
                });
            }

            PreSubscribedBroadcastSender {
                sender: tx_init,
                receivers,
            }
        }

        fn try_subscribe(&mut self) -> Option<ConditionalBroadcastReceiver> {
            self.receivers.pop()
        }

        fn send(&self) -> Result<usize, SendError<()>> {
            self.sender.send(())
        }
    }

    #[tokio::test]
    async fn test_pre_subscribed_broadcast() {
        let mut tx_shutdown = PreSubscribedBroadcastSender::new(2);
        let mut rx_shutdown_a = tx_shutdown.try_subscribe().unwrap();

        let a = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rx_shutdown_a.receiver.recv() => {
                        return 1
                    }

                    _ = async{}, if true => {
                        if rx_shutdown_a.received_signal().await {
                            return 1
                        }
                    }
                }
            }
        });

        let mut rx_shutdown_b = tx_shutdown.try_subscribe().unwrap();
        let rx_shutdown_c = tx_shutdown.try_subscribe();

        assert!(rx_shutdown_c.is_none());

        // send the shutdown signal before we start component b and started listening
        // for shutdown there
        assert!(tx_shutdown.send().is_ok());

        let b = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rx_shutdown_b.receiver.recv() => {
                        return 2
                    }

                    _ = async{}, if true => {
                        if rx_shutdown_b.received_signal().await {
                            return 2
                        }
                    }
                }
            }
        });

        // assert that both component a and b loops have exited, effectively shutting
        // down
        assert_eq!(a.await.unwrap() + b.await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_conditional_broadcast_receiver() {
        let mut tx_shutdown: PreSubscribedBroadcastSender = PreSubscribedBroadcastSender::new(2);
        let mut rx_shutdown = tx_shutdown.try_subscribe().unwrap();

        let a = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = async{}, if true => {
                        if rx_shutdown.received_signal().await {
                            return 1
                        }
                    }
                }
            }
        });

        assert!(tx_shutdown.send().is_ok());

        assert_eq!(a.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_connectivity() {
        // GIVEN
        let network_1 = build_network().unwrap();
        let network_2 = build_network().unwrap();
        let network_3 = build_network().unwrap();

        let registry = Registry::new();
        let metrics = NetworkConnectionMetrics::new("primary", &registry);

        // AND we connect to peer 2
        let peer_2 = network_1.connect(network_2.local_addr()).await.unwrap();

        let mut peer_types = HashMap::new();
        peer_types.insert(network_2.peer_id(), "other_network".to_string());
        peer_types.insert(network_3.peer_id(), "other_network".to_string());

        // WHEN bring up the monitor
        let (_h, statuses) =
            ConnectionMonitor::spawn(network_1.downgrade(), metrics.clone(), peer_types, None);

        // THEN peer 2 should be already connected
        assert_network_peers(metrics.clone(), 1).await;

        // AND we should have collected connection stats
        let mut labels = HashMap::new();
        let peer_2_str = format!("{peer_2}");
        labels.insert("peer_id", peer_2_str.as_str());
        assert_ne!(
            metrics
                .network_peer_rtt
                .get_metric_with(&labels)
                .unwrap()
                .get(),
            0
        );
        assert_eq!(
            *statuses.get(&peer_2).unwrap().value(),
            ConnectionStatus::Connected
        );

        // WHEN connect to peer 3
        let peer_3 = network_1.connect(network_3.local_addr()).await.unwrap();

        // THEN
        assert_network_peers(metrics.clone(), 2).await;
        assert_eq!(
            *statuses.get(&peer_3).unwrap().value(),
            ConnectionStatus::Connected
        );

        // AND disconnect peer 2
        network_1.disconnect(peer_2).unwrap();

        // THEN
        assert_network_peers(metrics.clone(), 1).await;
        assert_eq!(
            *statuses.get(&peer_2).unwrap().value(),
            ConnectionStatus::Disconnected
        );

        // AND disconnect peer 3
        network_1.disconnect(peer_3).unwrap();

        // THEN
        assert_network_peers(metrics.clone(), 0).await;
        assert_eq!(
            *statuses.get(&peer_3).unwrap().value(),
            ConnectionStatus::Disconnected
        );
    }

    async fn assert_network_peers(metrics: NetworkConnectionMetrics, value: i64) {
        let m = metrics.clone();
        timeout(Duration::from_secs(5), async move {
            while m.network_peers.get() != value {
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Timeout while waiting for connectivity results for value {}",
                value
            )
        });

        assert_eq!(metrics.network_peers.get(), value);
    }

    fn build_network() -> anyhow::Result<Network> {
        let network = Network::bind("localhost:0")
            .private_key(random_private_key())
            .server_name("test")
            .start(echo_service())?;
        Ok(network)
    }

    fn echo_service() -> BoxCloneService<Request<Bytes>, Response<Bytes>, Infallible> {
        let handle = move |request: Request<Bytes>| async move {
            let response = Response::new(request.into_body());
            Result::<Response<Bytes>, Infallible>::Ok(response)
        };

        tower::ServiceExt::boxed_clone(tower::service_fn(handle))
    }

    fn random_private_key() -> [u8; 32] {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut bytes[..]);

        bytes
    }
}
