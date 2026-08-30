// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use iota_metrics::spawn_monitored_task;
use parking_lot::{Mutex, RwLock};
use prometheus_filtered::IntCounter;
use starfish_config::AuthorityIndex;
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tracing::{debug, error, info};

use crate::{
    context::Context,
    dag_state::DagState,
    encoder::create_encoder,
    error::ConsensusError,
    network::{NetworkClient, NetworkService},
};

/// Returns whether the reset channel closed; otherwise records the reset and
/// clears the retry count.
fn observe_reset(
    changed: Result<(), watch::error::RecvError>,
    resets: &IntCounter,
    retries: &mut i64,
    peer: AuthorityIndex,
    peer_hostname: &str,
) -> bool {
    if changed.is_err() {
        return true;
    }
    debug!("Resetting block stream subscription to peer {peer} {peer_hostname}");
    resets.inc();
    *retries = 0;
    false
}

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
    block_stream_reset_sender: watch::Sender<()>,
    subscriptions: Arc<Mutex<Box<[Option<JoinHandle<()>>]>>>,
}

impl<C: NetworkClient, S: NetworkService> Subscriber<C, S> {
    pub(crate) fn new(
        context: Arc<Context>,
        network_client: Arc<C>,
        authority_service: Arc<S>,
        dag_state: Arc<RwLock<DagState>>,
        block_stream_reset_sender: watch::Sender<()>,
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
        self.context
            .peer_responsiveness
            .clear_streaming_block_delivery(peer);
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
        mut block_stream_reset_receiver: watch::Receiver<()>,
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
                .peer_responsiveness
                .clear_streaming_block_delivery(peer);
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
                            if observe_reset(reset, &block_stream_resets, &mut retries, peer, peer_hostname)
                            {
                                return;
                            }
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

            let last_received = dag_state.read().resume_round_for_authority(peer);
            // Wrap subscribe_block_bundles in a timeout and increment metric on timeout
            let subscribe_future =
                network_client.subscribe_block_bundles(peer, last_received, MAX_RETRY_INTERVAL);
            let subscribe_result = tokio::select! {
                biased;
                reset = block_stream_reset_receiver.changed() => {
                        if observe_reset(reset, &block_stream_resets, &mut retries, peer, peer_hostname)
                        {
                            return;
                        }
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
                            if observe_reset(reset, &block_stream_resets, &mut retries, peer, peer_hostname)
                            {
                                return;
                            }
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

    /// How the fake peer answers subscriptions.
    enum StreamMode {
        /// Ten bundles, one per millisecond.
        Paced,
        /// Never yields, so the subscriber stays parked on the stream.
        Pending,
        /// Ten bundles at once on the first subscription, then never yields, so
        /// a reconnect cannot add to the handled count.
        BufferedOnce,
        /// Fails the first `n` subscribe calls, then never yields.
        FailThenPending(usize),
    }

    struct SubscriberTestClient {
        last_received_rounds: Mutex<Vec<Round>>,
        subscription_attempts_sender: watch::Sender<usize>,
        stream_mode: StreamMode,
    }

    impl SubscriberTestClient {
        fn new(stream_mode: StreamMode) -> Self {
            let (subscription_attempts_sender, _) = watch::channel(0);
            Self {
                last_received_rounds: Mutex::new(Vec::new()),
                subscription_attempts_sender,
                stream_mode,
            }
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
            match self.stream_mode {
                StreamMode::FailThenPending(failures) if attempts <= failures => Err(
                    ConsensusError::NetworkRequest("injected failure".to_owned()),
                ),
                StreamMode::Pending | StreamMode::FailThenPending(_) => {
                    Ok(Box::pin(stream::pending()))
                }
                StreamMode::Paced => {
                    let block_stream = stream::unfold((), |_| async {
                        sleep(Duration::from_millis(1)).await;
                        Some((test_bundle(), ()))
                    })
                    .take(10);
                    Ok(Box::pin(block_stream))
                }
                StreamMode::BufferedOnce if attempts == 1 => Ok(Box::pin(stream::iter(
                    std::iter::repeat_with(test_bundle).take(10),
                ))),
                StreamMode::BufferedOnce => Ok(Box::pin(stream::pending())),
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
        timeout(
            Duration::from_secs(5),
            attempts.wait_for(|attempts| *attempts >= expected),
        )
        .await
        .unwrap_or_else(|_| panic!("subscriber did not make {expected} subscription attempts"))
        .expect("the sender outlives the test");
    }

    struct Fixture {
        context: Arc<Context>,
        network_client: Arc<SubscriberTestClient>,
        authority_service: Arc<Mutex<TestService>>,
        dag_state: Arc<RwLock<DagState>>,
        reset_sender: watch::Sender<()>,
        subscriber: Subscriber<SubscriberTestClient, Mutex<TestService>>,
        peer: AuthorityIndex,
    }

    /// Wires a subscriber against `service` and `mode`, stopping short of
    /// subscribing so a test can seed `dag_state` first.
    fn fixture(context: Arc<Context>, service: TestService, mode: StreamMode) -> Fixture {
        telemetry_subscribers::init_for_testing();
        let network_client = Arc::new(SubscriberTestClient::new(mode));
        let authority_service = Arc::new(Mutex::new(service));
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let (reset_sender, _) = watch::channel(());
        let subscriber = Subscriber::new(
            context.clone(),
            network_client.clone(),
            authority_service.clone(),
            dag_state.clone(),
            reset_sender.clone(),
        );
        let peer = context.committee.to_authority_index(2).unwrap();
        Fixture {
            context,
            network_client,
            authority_service,
            dag_state,
            reset_sender,
            subscriber,
            peer,
        }
    }

    fn test_context(committee_size: usize) -> Arc<Context> {
        let (context, _keys) = Context::new_for_test(committee_size);
        Arc::new(context)
    }

    fn header_for(peer: AuthorityIndex, round: Round) -> VerifiedBlockHeader {
        VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, peer.value() as u8).build())
    }

    #[test]
    fn unsubscribe_clears_streaming_block_latency() {
        let f = fixture(test_context(4), TestService::new(), StreamMode::Pending);
        f.context
            .peer_responsiveness
            .record_streaming_block_delivery(f.peer, Duration::from_millis(10));

        f.subscriber.unsubscribe(f.peer);

        assert_eq!(
            f.context
                .peer_responsiveness
                .streaming_block_latency_ms(f.peer),
            None
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_retries() {
        let f = fixture(test_context(4), TestService::new(), StreamMode::Paced);
        f.subscriber.subscribe(f.peer);
        wait_for_subscription_attempts(&f.network_client, 1).await;

        f.dag_state
            .write()
            .accept_block_headers(vec![header_for(f.peer, 7)], DataSource::BlockBundleStream);
        wait_for_subscription_attempts(&f.network_client, 2).await;

        // Wait for enough block bundles received.
        for _ in 0..10 {
            sleep(Duration::from_secs(1)).await;
            let service = f.authority_service.lock();
            if service.handle_subscribed_block_bundle.len() >= 100 {
                break;
            }
        }

        // Even if the stream ends after 10 blocks, the subscriber should retry and get
        // enough blocks eventually.
        let service = f.authority_service.lock();
        assert!(service.handle_subscribed_block_bundle.len() >= 100);
        for (p, block) in service.handle_subscribed_block_bundle.iter() {
            assert_eq!(*p, f.peer);
            assert_eq!(*block, test_bundle());
        }
        let rounds = f.network_client.last_received_rounds();
        assert!(
            rounds.len() > 1 && rounds[1..].iter().all(|round| *round == 7),
            "reconnects should resume from the peer's latest header: {rounds:?}"
        );
        drop(service);
        f.subscriber.stop();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_resets_stream_with_latest_cursor() {
        let f = fixture(test_context(4), TestService::new(), StreamMode::Pending);
        f.subscriber.subscribe(f.peer);
        wait_for_subscription_attempts(&f.network_client, 1).await;

        f.dag_state
            .write()
            .accept_block_headers(vec![header_for(f.peer, 11)], DataSource::BlockBundleStream);
        f.reset_sender.send_replace(());

        wait_for_subscription_attempts(&f.network_client, 2).await;
        assert_eq!(f.network_client.last_received_rounds(), vec![0, 11]);
        f.subscriber.stop();
    }

    /// A reset observed mid-bundle must not cancel the handler that is running.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_finishes_bundle_handler_before_reset() {
        let (service, gate) = TestService::with_bundle_handler_gate();
        let f = fixture(test_context(4), service, StreamMode::Paced);
        f.subscriber.subscribe(f.peer);
        gate.started.notified().await;

        f.reset_sender.send_replace(());
        sleep(Duration::from_secs(1)).await;
        assert_eq!(
            f.network_client.last_received_rounds().len(),
            1,
            "a reset must not reconnect while the bundle handler is still running"
        );

        gate.release.notify_one();
        wait_for_subscription_attempts(&f.network_client, 2).await;
        f.subscriber.stop();
    }

    /// A reset while the subscriber is waiting out a retry backoff reconnects
    /// straight away instead of sitting out the rest of the delay.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_reset_short_circuits_retry_backoff() {
        // Four failed attempts put the next one into the delayed-retry branch.
        let f = fixture(
            test_context(4),
            TestService::new(),
            StreamMode::FailThenPending(4),
        );
        f.subscriber.subscribe(f.peer);
        wait_for_subscription_attempts(&f.network_client, 4).await;
        // Let the subscriber reach the backoff sleep before resetting it.
        sleep(Duration::from_millis(1)).await;

        let before_reset = tokio::time::Instant::now();
        f.reset_sender.send_replace(());
        wait_for_subscription_attempts(&f.network_client, 5).await;

        let waited = before_reset.elapsed();
        assert!(
            waited < Duration::from_millis(50),
            "reset should not wait out the backoff, waited {waited:?}"
        );
        f.subscriber.stop();
    }

    /// Bundles already buffered on the stream are dropped by a reset rather
    /// than handled against the reinitialized state.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_discards_buffered_bundles_on_reset() {
        let (service, gate) = TestService::with_bundle_handler_gate();
        let f = fixture(test_context(4), service, StreamMode::BufferedOnce);
        f.subscriber.subscribe(f.peer);
        // The first bundle is in the handler, the other nine sit buffered.
        gate.started.notified().await;

        f.reset_sender.send_replace(());
        gate.release.notify_one();
        wait_for_subscription_attempts(&f.network_client, 2).await;

        assert_eq!(
            f.authority_service
                .lock()
                .handle_subscribed_block_bundle
                .len(),
            1,
            "buffered bundles should be discarded by the reset"
        );
        f.subscriber.stop();
    }

    /// The subscription cursor never drops below the GC round: blocks at or
    /// below it can no longer be sequenced, so replaying them is wasted work.
    /// Above the GC round the peer's own latest header still wins.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn subscriber_does_not_resume_below_gc_round() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.protocol_config.set_gc_depth_for_testing(5);
        let f = fixture(Arc::new(context), TestService::new(), StreamMode::Pending);
        // A commit led at round 30 with gc_depth 5 puts the GC round at 20.
        f.dag_state
            .write()
            .set_last_commit(TrustedCommit::new_for_test(
                &f.context,
                1,
                CommitDigest::MIN,
                f.context.clock.timestamp_utc_ms(),
                BlockRef::new(30, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                vec![],
                vec![],
            ));
        f.subscriber.subscribe(f.peer);
        wait_for_subscription_attempts(&f.network_client, 1).await;

        f.dag_state
            .write()
            .accept_block_headers(vec![header_for(f.peer, 25)], DataSource::BlockBundleStream);
        f.reset_sender.send_replace(());
        wait_for_subscription_attempts(&f.network_client, 2).await;

        assert_eq!(f.network_client.last_received_rounds(), vec![20, 25]);
        f.subscriber.stop();
    }
}
