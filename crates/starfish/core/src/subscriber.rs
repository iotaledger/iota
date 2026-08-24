// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use iota_metrics::spawn_monitored_task;
use parking_lot::{Mutex, RwLock};
use starfish_config::AuthorityIndex;
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tracing::{debug, error, info};

use crate::{
    block_header::BlockHeaderAPI as _,
    context::Context,
    dag_state::DagState,
    encoder::create_encoder,
    error::ConsensusError,
    network::{NetworkClient, NetworkService},
};

/// Subscriber manages the block stream subscriptions to other peers, taking
/// care of retrying when subscription streams break. Blocks returned from the
/// peer are sent to the authority service for processing.
/// Currently subscription management for individual peer is not exposed, but it
/// could become useful in future.
pub(crate) struct Subscriber<C: NetworkClient, S: NetworkService> {
    context: Arc<Context>,
    network_client: Arc<C>,
    authority_service: Arc<S>,
    dag_state: Arc<RwLock<DagState>>,
    block_stream_reset_sender: watch::Sender<u64>,
    subscriptions: Arc<Mutex<Box<[Option<JoinHandle<()>>]>>>,
}

impl<C: NetworkClient, S: NetworkService> Subscriber<C, S> {
    pub(crate) fn new(
        context: Arc<Context>,
        network_client: Arc<C>,
        authority_service: Arc<S>,
        dag_state: Arc<RwLock<DagState>>,
        block_stream_reset_sender: watch::Sender<u64>,
    ) -> Self {
        // Drop label combos left over from previous epochs whose hostnames
        // are no longer in the current committee — otherwise IntGaugeVec
        // keeps re-emitting the last value (typically 1) forever.
        context.metrics.node_metrics.subscribed_to.reset();
        let subscriptions = (0..context.committee.size())
            .map(|_| None)
            .collect::<Vec<_>>();
        Self {
            context,
            network_client,
            authority_service,
            dag_state,
            block_stream_reset_sender,
            subscriptions: Arc::new(Mutex::new(subscriptions.into_boxed_slice())),
        }
    }

    pub(crate) fn subscribe(&self, peer: AuthorityIndex) {
        if peer == self.context.own_index {
            error!("Attempt to subscribe to own validator {peer} is ignored!");
            return;
        }
        let context = self.context.clone();
        let network_client = self.network_client.clone();
        let authority_service = self.authority_service.clone();
        let dag_state = self.dag_state.clone();
        let block_stream_reset_receiver = self.block_stream_reset_sender.subscribe();

        let mut subscriptions = self.subscriptions.lock();
        self.unsubscribe_locked(peer, &mut subscriptions[peer.value()]);
        subscriptions[peer.value()] = Some(spawn_monitored_task!(Self::subscription_loop(
            context,
            network_client,
            authority_service,
            dag_state,
            peer,
            block_stream_reset_receiver,
        )));
    }

    pub(crate) fn stop(&self) {
        let mut subscriptions = self.subscriptions.lock();
        for (peer, _) in self.context.committee.authorities() {
            self.unsubscribe_locked(peer, &mut subscriptions[peer.value()]);
        }
    }

    /// Unsubscribe from a specific peer. Used for testing scenarios where
    /// we need to simulate network partitions without stopping the validator.
    #[cfg(test)]
    pub(crate) fn unsubscribe(&self, peer: AuthorityIndex) {
        let mut subscriptions = self.subscriptions.lock();
        self.unsubscribe_locked(peer, &mut subscriptions[peer.value()]);
    }

    fn unsubscribe_locked(&self, peer: AuthorityIndex, subscription: &mut Option<JoinHandle<()>>) {
        let peer_hostname = &self.context.committee.authority(peer).hostname;
        if let Some(subscription) = subscription.take() {
            subscription.abort();
        }
        // There is a race between shutting down the subscription task and clearing the
        // metric here. TODO: fix the race when unsubscribe_locked() gets called
        // outside of stop().
        self.context
            .metrics
            .node_metrics
            .subscribed_to
            .with_label_values(&[peer_hostname])
            .set(0);
    }

    async fn subscription_loop(
        context: Arc<Context>,
        network_client: Arc<C>,
        authority_service: Arc<S>,
        dag_state: Arc<RwLock<DagState>>,
        peer: AuthorityIndex,
        mut block_stream_reset_receiver: watch::Receiver<u64>,
    ) {
        const IMMEDIATE_RETRIES: i64 = 3;
        // When not immediately retrying, limit retry delay between 100ms and 10s.
        const INITIAL_RETRY_INTERVAL: Duration = Duration::from_millis(100);
        const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(10);
        const RETRY_INTERVAL_MULTIPLIER: f32 = 1.2;
        let peer_hostname = &context.committee.authority(peer).hostname;
        let mut retries: i64 = 0;
        let mut delay = INITIAL_RETRY_INTERVAL;
        let block_stream_resets = context
            .metrics
            .node_metrics
            .block_stream_resets
            .with_label_values(&[peer_hostname]);

        let mut encoder = create_encoder(&context);

        'subscription: loop {
            context
                .metrics
                .node_metrics
                .subscribed_to
                .with_label_values(&[peer_hostname])
                .set(0);

            if retries > IMMEDIATE_RETRIES {
                debug!(
                    "Delaying retry {} of peer {} subscription, in {} seconds",
                    retries,
                    peer_hostname,
                    delay.as_secs_f32(),
                );
                tokio::select! {
                    biased;
                    reset = block_stream_reset_receiver.changed() => {
                        if reset.is_err() {
                            return;
                        }
                        block_stream_resets.inc();
                        retries = 0;
                        continue 'subscription;
                    }
                    _ = sleep(delay) => {}
                }
                // Update delay for the next retry.
                delay = delay
                    .mul_f32(RETRY_INTERVAL_MULTIPLIER)
                    .min(MAX_RETRY_INTERVAL);
            } else if retries > 0 {
                // Retry immediately, but still yield to avoid monopolizing the thread.
                tokio::task::yield_now().await;
            } else {
                // First attempt, reset delay for next retries but no waiting.
                delay = INITIAL_RETRY_INTERVAL;
            }
            retries += 1;

            // Blocks at or below the GC round can no longer be sequenced, so
            // never ask a peer to resend from further back than that.
            let last_received = {
                let dag_state = dag_state.read();
                dag_state
                    .get_last_block_header_for_authority(peer)
                    .round()
                    .max(dag_state.gc_round_for_last_commit())
            };
            // Wrap subscribe_block_bundles in a timeout and increment metric on timeout
            let subscribe_future =
                network_client.subscribe_block_bundles(peer, last_received, MAX_RETRY_INTERVAL);
            let subscribe_result = tokio::select! {
                biased;
                reset = block_stream_reset_receiver.changed() => {
                    if reset.is_err() {
                        return;
                    }
                    block_stream_resets.inc();
                    retries = 0;
                    continue 'subscription;
                }
                result = timeout(MAX_RETRY_INTERVAL * 5, subscribe_future) => result,
            };
            let mut block_bundles = match subscribe_result {
                Ok(inner_result) => match inner_result {
                    Ok(blocks) => {
                        debug!(
                            "Subscribed to peer {} {} after {} attempts",
                            peer, peer_hostname, retries
                        );
                        context
                            .metrics
                            .node_metrics
                            .subscriber_connection_attempts
                            .with_label_values(&[peer_hostname.as_str(), "success"])
                            .inc();
                        blocks
                    }
                    Err(e) => {
                        debug!(
                            "Failed to subscribe to blocks from peer {} {}: {}",
                            peer, peer_hostname, e
                        );
                        context
                            .metrics
                            .node_metrics
                            .subscriber_connection_attempts
                            .with_label_values(&[peer_hostname.as_str(), "failure"])
                            .inc();
                        continue 'subscription;
                    }
                },
                Err(_) => {
                    debug!(
                        "Timeout subscribing to blocks from peer {} {}",
                        peer, peer_hostname
                    );
                    context
                        .metrics
                        .node_metrics
                        .subscriber_connection_attempts
                        .with_label_values(&[peer_hostname.as_str(), "timeout"])
                        .inc();
                    continue 'subscription;
                }
            };

            // Now can consider the subscription successful
            context
                .metrics
                .node_metrics
                .subscribed_to
                .with_label_values(&[peer_hostname])
                .set(1);

            'stream: loop {
                // Observe a reset only between bundles: wrapping the handler
                // below in this select would cancel it mid-bundle.
                let next_block = tokio::select! {
                    biased;
                    reset = block_stream_reset_receiver.changed() => {
                        if reset.is_err() {
                            return;
                        }
                        debug!("Resetting block stream subscription to peer {peer} {peer_hostname}");
                        block_stream_resets.inc();
                        retries = 0;
                        continue 'subscription;
                    }
                    block = block_bundles.next() => block,
                };
                match next_block {
                    Some(block) => {
                        context
                            .metrics
                            .node_metrics
                            .subscribed_block_bundles
                            .with_label_values(&[peer_hostname])
                            .inc();
                        let result = authority_service
                            .handle_subscribed_block_bundle(peer, block, &mut encoder)
                            .await;
                        if let Err(e) = result {
                            match e {
                                ConsensusError::BlockRejected { block_ref, reason } => {
                                    debug!(
                                        "Failed to process block from peer {} {} for block {:?}: {}",
                                        peer, peer_hostname, block_ref, reason
                                    );
                                }
                                _ => {
                                    info!(
                                        "Invalid block received from peer {} {}: {}",
                                        peer, peer_hostname, e
                                    );
                                }
                            }
                        }
                        // Reset retries when a block is received.
                        retries = 0;
                    }
                    None => {
                        debug!(
                            "Subscription to blocks from peer {} {} ended",
                            peer, peer_hostname
                        );
                        retries += 1;
                        break 'stream;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::stream;

    use super::*;
    use crate::{
        Round,
        block_header::{BlockHeaderDigest, BlockRef, TestBlockHeader, VerifiedBlockHeader},
        commit::{CommitDigest, CommitRange, TrustedCommit},
        dag_state::DataSource,
        error::{ConsensusError, ConsensusResult},
        network::{BlockBundleStream, SerializedBlockBundle, test_network::TestService},
        storage::mem_store::MemStore,
        transaction_ref::TransactionRef,
    };

    /// How the fake peer feeds bundles once a subscription succeeds.
    enum StreamMode {
        /// Ten bundles, one per millisecond.
        Paced,
        /// Never yields, so the subscriber stays parked on the stream.
        Pending,
        /// Ten bundles at once on the first subscription, then a stream that
        /// never yields, so a reconnect cannot add to the handled count.
        BufferedOnce,
    }

    struct SubscriberTestClient {
        last_received_rounds: Mutex<Vec<Round>>,
        subscription_attempts_sender: watch::Sender<usize>,
        stream_mode: StreamMode,
        initial_failures: usize,
    }

    impl SubscriberTestClient {
        fn new(stream_mode: StreamMode) -> Self {
            let (subscription_attempts_sender, _) = watch::channel(0);
            Self {
                last_received_rounds: Mutex::new(Vec::new()),
                subscription_attempts_sender,
                stream_mode,
                initial_failures: 0,
            }
        }

        fn with_initial_failures(mut self, initial_failures: usize) -> Self {
            self.initial_failures = initial_failures;
            self
        }

        fn last_received_rounds(&self) -> Vec<Round> {
            self.last_received_rounds.lock().clone()
        }
    }

    fn test_bundle() -> SerializedBlockBundle {
        SerializedBlockBundle {
            serialized_block_bundle: Bytes::from(vec![1u8; 8]),
        }
    }

    #[async_trait]
    impl NetworkClient for SubscriberTestClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
            let attempts = {
                let mut last_received_rounds = self.last_received_rounds.lock();
                last_received_rounds.push(last_received);
                last_received_rounds.len()
            };
            self.subscription_attempts_sender.send_replace(attempts);
            if attempts <= self.initial_failures {
                return Err(ConsensusError::NetworkRequest(
                    "injected failure".to_owned(),
                ));
            }
            match self.stream_mode {
                StreamMode::Pending => Ok(Box::pin(stream::pending())),
                StreamMode::Paced => {
                    let block_stream = stream::unfold((), |_| async {
                        sleep(Duration::from_millis(1)).await;
                        Some((test_bundle(), ()))
                    })
                    .take(10);
                    Ok(Box::pin(block_stream))
                }
                StreamMode::BufferedOnce => {
                    if attempts > self.initial_failures + 1 {
                        return Ok(Box::pin(stream::pending()));
                    }
                    Ok(Box::pin(stream::iter(
                        std::iter::repeat_with(test_bundle).take(10),
                    )))
                }
            }
        }

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _transaction_refs: Vec<TransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_block_headers(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_block_headers(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_commits_and_transactions(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>, Option<ConsensusError>)> {
            unimplemented!("Unimplemented")
        }
    }

    async fn wait_for_subscription_attempts(
        network_client: &SubscriberTestClient,
        expected: usize,
    ) {
        let mut attempts = network_client.subscription_attempts_sender.subscribe();
        timeout(Duration::from_secs(5), async {
            loop {
                if *attempts.borrow() >= expected {
                    return;
                }
                attempts.changed().await.unwrap();
            }
        })
        .await
        .unwrap_or_else(|_| panic!("subscriber did not make {expected} subscription attempts"));
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_retries() {
        telemetry_subscribers::init_for_testing();
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let authority_service = Arc::new(Mutex::new(TestService::new()));
        let network_client = Arc::new(SubscriberTestClient::new(StreamMode::Paced));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service.clone(),
            dag_state.clone(),
            block_stream_reset_sender,
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        wait_for_subscription_attempts(&network_client, 1).await;

        let header =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(7, peer.value() as u8).build());
        dag_state
            .write()
            .accept_block_headers(vec![header], DataSource::BlockBundleStream);
        wait_for_subscription_attempts(&network_client, 2).await;

        // Wait for enough block bundles received.
        for _ in 0..10 {
            sleep(Duration::from_secs(1)).await;
            let service = authority_service.lock();
            if service.handle_subscribed_block_bundle.len() >= 100 {
                break;
            }
        }

        // Even if the stream ends after 10 blocks, the subscriber should retry and get
        // enough blocks eventually.
        let service = authority_service.lock();
        assert!(service.handle_subscribed_block_bundle.len() >= 100);
        for (p, block) in service.handle_subscribed_block_bundle.iter() {
            assert_eq!(*p, peer);
            assert_eq!(
                *block,
                SerializedBlockBundle {
                    serialized_block_bundle: Bytes::from(vec![1u8; 8]),
                }
            );
        }
        let rounds = network_client.last_received_rounds();
        assert!(
            rounds.len() > 1 && rounds[1..].iter().all(|round| *round == 7),
            "reconnects should resume from the peer's latest header: {rounds:?}"
        );
        drop(service);
        subscriber.stop();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_resets_stream_with_latest_cursor() {
        telemetry_subscribers::init_for_testing();
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let authority_service = Arc::new(Mutex::new(TestService::new()));
        let network_client = Arc::new(SubscriberTestClient::new(StreamMode::Pending));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service,
            dag_state.clone(),
            block_stream_reset_sender.clone(),
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        wait_for_subscription_attempts(&network_client, 1).await;

        let header =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(11, peer.value() as u8).build());
        dag_state
            .write()
            .accept_block_headers(vec![header], DataSource::BlockBundleStream);
        block_stream_reset_sender.send_modify(|generation| {
            *generation += 1;
        });

        wait_for_subscription_attempts(&network_client, 2).await;
        assert_eq!(network_client.last_received_rounds(), vec![0, 11]);
        subscriber.stop();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_finishes_bundle_handler_before_reset() {
        telemetry_subscribers::init_for_testing();
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let handler_started = Arc::new(tokio::sync::Notify::new());
        let handler_release = Arc::new(tokio::sync::Notify::new());
        let mut service = TestService::new();
        service.block_bundle_handler_started = Some(handler_started.clone());
        service.block_bundle_handler_release = Some(handler_release.clone());
        let authority_service = Arc::new(Mutex::new(service));
        let network_client = Arc::new(SubscriberTestClient::new(StreamMode::Paced));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service,
            dag_state,
            block_stream_reset_sender.clone(),
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        handler_started.notified().await;

        block_stream_reset_sender.send_modify(|generation| {
            *generation += 1;
        });
        assert!(
            timeout(
                Duration::from_secs(1),
                wait_for_subscription_attempts(&network_client, 2)
            )
            .await
            .is_err()
        );

        handler_release.notify_one();
        wait_for_subscription_attempts(&network_client, 2).await;
        subscriber.stop();
    }

    /// A reset while the subscriber is waiting out a retry backoff reconnects
    /// straight away instead of sitting out the rest of the delay.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_reset_short_circuits_retry_backoff() {
        telemetry_subscribers::init_for_testing();
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let authority_service = Arc::new(Mutex::new(TestService::new()));
        // Four failed attempts put the next one into the delayed-retry branch.
        let network_client =
            Arc::new(SubscriberTestClient::new(StreamMode::Pending).with_initial_failures(4));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service,
            dag_state,
            block_stream_reset_sender.clone(),
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        wait_for_subscription_attempts(&network_client, 4).await;
        // Let the subscriber reach the backoff sleep before resetting it.
        sleep(Duration::from_millis(1)).await;

        let before_reset = tokio::time::Instant::now();
        block_stream_reset_sender.send_modify(|generation| {
            *generation += 1;
        });
        wait_for_subscription_attempts(&network_client, 5).await;

        let waited = before_reset.elapsed();
        assert!(
            waited < Duration::from_millis(50),
            "reset should not wait out the backoff, waited {waited:?}"
        );
        subscriber.stop();
    }

    /// Bundles already buffered on the stream are dropped by a reset rather
    /// than handled against the reinitialized state.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_discards_buffered_bundles_on_reset() {
        telemetry_subscribers::init_for_testing();
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let handler_started = Arc::new(tokio::sync::Notify::new());
        let handler_release = Arc::new(tokio::sync::Notify::new());
        let mut service = TestService::new();
        service.block_bundle_handler_started = Some(handler_started.clone());
        service.block_bundle_handler_release = Some(handler_release.clone());
        let authority_service = Arc::new(Mutex::new(service));
        let network_client = Arc::new(SubscriberTestClient::new(StreamMode::BufferedOnce));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service.clone(),
            dag_state,
            block_stream_reset_sender.clone(),
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        // The first bundle is in the handler, the other nine sit buffered.
        handler_started.notified().await;

        block_stream_reset_sender.send_modify(|generation| {
            *generation += 1;
        });
        handler_release.notify_one();
        wait_for_subscription_attempts(&network_client, 2).await;

        assert_eq!(
            authority_service
                .lock()
                .handle_subscribed_block_bundle
                .len(),
            1,
            "buffered bundles should be discarded by the reset"
        );
        subscriber.stop();
    }

    /// The subscription cursor never drops below the GC round: blocks at or
    /// below it can no longer be sequenced, so replaying them is wasted work.
    /// Above the GC round the peer's own latest header still wins.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_does_not_resume_below_gc_round() {
        telemetry_subscribers::init_for_testing();
        let (mut context, _keys) = Context::new_for_test(4);
        context.protocol_config.set_gc_depth_for_testing(5);
        let context = Arc::new(context);
        let authority_service = Arc::new(Mutex::new(TestService::new()));
        let network_client = Arc::new(SubscriberTestClient::new(StreamMode::Pending));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        // A commit led at round 30 with gc_depth 5 puts the GC round at 20.
        dag_state
            .write()
            .set_last_commit(TrustedCommit::new_for_test(
                &context,
                1,
                CommitDigest::MIN,
                context.clock.timestamp_utc_ms(),
                BlockRef::new(30, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                vec![],
                vec![],
            ));
        let (block_stream_reset_sender, _) = watch::channel(0_u64);
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service,
            dag_state.clone(),
            block_stream_reset_sender.clone(),
        );

        let peer = context.committee.to_authority_index(2).unwrap();
        subscriber.subscribe(peer);
        wait_for_subscription_attempts(&network_client, 1).await;

        let header =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(25, peer.value() as u8).build());
        dag_state
            .write()
            .accept_block_headers(vec![header], DataSource::BlockBundleStream);
        block_stream_reset_sender.send_modify(|generation| {
            *generation += 1;
        });
        wait_for_subscription_attempts(&network_client, 2).await;

        assert_eq!(network_client.last_received_rounds(), vec![20, 25]);
        subscriber.stop();
    }
}
