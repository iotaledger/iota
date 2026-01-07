// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{
        broadcast,
        oneshot::{Receiver, Sender},
        watch,
    },
    task::JoinHandle,
    time::{Instant, sleep_until},
};
use tracing::{debug, warn};

use crate::{
    BlockHeaderAPI,
    block_header::{Round, VerifiedBlock},
    context::Context,
    core::{CoreSignalsReceivers, ReasonToCreateBlock},
    core_thread::CoreThreadDispatcher,
    transactions_synchronizer::TransactionsSynchronizerHandle,
};

pub(crate) struct LeaderTimeoutTaskHandle {
    handle: JoinHandle<()>,
    stop: Sender<()>,
}

impl LeaderTimeoutTaskHandle {
    pub async fn stop(self) {
        self.stop.send(()).ok();
        self.handle.await.ok();
    }
}

pub(crate) struct LeaderTimeoutTask<D: CoreThreadDispatcher> {
    dispatcher: Arc<D>,
    transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
    new_round_receiver: watch::Receiver<Round>,
    new_block_receiver: broadcast::Receiver<VerifiedBlock>,
    leader_timeout: Duration,
    min_block_delay: Duration,
    stop: Receiver<()>,
}

impl<D: CoreThreadDispatcher> LeaderTimeoutTask<D> {
    /// Starts the leader timeout task, which monitors and manages the leader
    /// election timeout mechanism.
    pub fn start(
        dispatcher: Arc<D>,
        transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
        signals_receivers: &CoreSignalsReceivers,
        context: Arc<Context>,
    ) -> LeaderTimeoutTaskHandle {
        let (stop_sender, stop) = tokio::sync::oneshot::channel();
        let mut me = Self {
            dispatcher,
            transactions_synchronizer,
            stop,
            new_block_receiver: signals_receivers.block_broadcast_receiver(),
            new_round_receiver: signals_receivers.new_round_receiver(),
            leader_timeout: context.parameters.leader_timeout,
            min_block_delay: context.parameters.min_block_delay,
        };
        let handle = tokio::spawn(async move { me.run().await });

        LeaderTimeoutTaskHandle {
            handle,
            stop: stop_sender,
        }
    }

    /// Runs the leader timeout task, managing the leader election timeout
    /// mechanism in an asynchronous loop.
    /// This mechanism ensures that if the current leader fails to produce a new
    /// block within the specified timeout, the task forces the creation of a
    /// new block, maintaining the continuity and robustness of the leader
    /// election process.
    /// In addition, if min block delay timeout is expired it attempts to
    /// non-forcefully create a new block.
    async fn run(&mut self) {
        debug!("LeaderTimeoutTask is running");
        let new_clock_round = &mut self.new_round_receiver;
        let new_block = &mut self.new_block_receiver.resubscribe();
        let mut clock_round: Round = *new_clock_round.borrow_and_update();
        let mut last_own_block_round: Option<Round> = None;
        let mut min_block_delay_timed_out = false;
        let mut max_leader_round_timed_out = false;
        let timer_start = Instant::now();
        let min_block_delay_timeout = sleep_until(timer_start + self.min_block_delay);
        let max_leader_timeout = sleep_until(timer_start + self.leader_timeout);

        tokio::pin!(min_block_delay_timeout);
        tokio::pin!(max_leader_timeout);

        loop {
            tokio::select! {
                // When the min block delay timer expires, then we attempt to trigger the creation of a new block.
                // If we already timed out before then, the branch gets disabled so we don't attempt
                // all the time to produce already produced blocks for that round.

                () = &mut min_block_delay_timeout, if !min_block_delay_timed_out && last_own_block_round.is_some() => {
                    let next_round: Round = last_own_block_round.expect("We should expect some last own round") + 1;
                    match self.dispatcher.new_block(next_round, ReasonToCreateBlock::MinBlockDelayTimeout).await {
                        Ok(missing_committed_txns) => {
                            if !missing_committed_txns.is_empty() {
                                debug!(
                                    "Missing committed transactions after creating new block: {:?}",
                                    missing_committed_txns
                                );
                                if let Err(err) = self.transactions_synchronizer
                                    .fetch_transactions(missing_committed_txns)
                                    .await
                                {
                                    warn!(
                                        "Error while trying to fetch missing transactions via transactions synchronizer: {err}"
                                    );
                                }
                            }
                        },
                        Err(err) => {
                            warn!("Error received while calling dispatcher, probably dispatcher is shutting down, will now exit: {err:?}");
                            return;
                        }
                    }
                    min_block_delay_timed_out = true;
                },
                // When the max leader timer expires then we attempt to trigger the creation of a new block. This
                // call is made with reason MaxLeaderTimeout to bypass any checks that allow to propose immediately if block
                // not already produced.
                () = &mut max_leader_timeout, if !max_leader_round_timed_out => {
                    match self.dispatcher.new_block(clock_round, ReasonToCreateBlock::MaxLeaderTimeout).await {
                        Ok(missing_committed_txns) => {
                            if !missing_committed_txns.is_empty() {
                                debug!(
                                    "Missing committed transactions after creating new block: {:?}",
                                    missing_committed_txns
                                );
                                if let Err(err) = self.transactions_synchronizer
                                    .fetch_transactions(missing_committed_txns)
                                    .await
                                {
                                    warn!(
                                        "Error while trying to fetch missing transactions via transactions synchronizer: {err}"
                                    );
                                }
                            }
                        }
                        Err(err) =>  {
                            warn!("Error received while calling dispatcher, probably dispatcher is shutting down, will now exit: {err:?}");
                            return;
                        }
                    }
                    max_leader_round_timed_out = true;
                }

                // A clock round has been advanced. Reset the leader timeout.
                Ok(_) = new_clock_round.changed() => {
                    clock_round = *new_clock_round.borrow_and_update();
                    debug!("New clock round has been received {clock_round}, resetting timer");
                    let _span = tracing::trace_span!("new_consensus_round_received", round = ?clock_round).entered();

                    max_leader_round_timed_out = false;

                    let now = Instant::now();

                    max_leader_timeout
                    .as_mut()
                    .reset(now + self.leader_timeout);
                },
                 // A new block was created. Set a timer in min_block_delay
                Ok(block) = new_block.recv() => {
                    debug!("New block {:?} was created and seen in leader timeout task", block.verified_block_header);
                    last_own_block_round = Some(block.round());

                    min_block_delay_timed_out = false;

                    let now = Instant::now();
                    min_block_delay_timeout.as_mut().reset(now + self.min_block_delay);
                },
                _ = &mut self.stop => {
                    debug!("Stop signal has been received, now shutting down");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use bytes::Bytes;
    use parking_lot::RwLock;
    use starfish_config::{AuthorityIndex, Parameters};
    use tokio::time::{Instant, sleep};

    use crate::{
        BlockRef, Round, TestBlockHeader,
        block_header::VerifiedBlock,
        commit::CommitRange,
        context::Context,
        core::{CoreSignals, ReasonToCreateBlock},
        core_thread::tests::MockCoreThreadDispatcher,
        dag_state::DagState,
        encoder::create_encoder,
        error::ConsensusResult,
        leader_timeout::LeaderTimeoutTask,
        network::{BlockBundleStream, NetworkClient},
        storage::mem_store::MemStore,
        transaction_ref::GenericTransactionRef,
        transactions_synchronizer::TransactionsSynchronizer,
    };

    #[derive(Default)]
    struct FakeNetworkClient {}

    #[async_trait::async_trait]
    impl NetworkClient for FakeNetworkClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<GenericTransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        // Returns a vector of serialized block headers
        async fn fetch_block_headers(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!();
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
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn basic_leader_timeout() {
        telemetry_subscribers::init_for_testing();
        let (context, _signers) = Context::new_for_test(4);
        let dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let leader_timeout = Duration::from_millis(500);
        let min_block_delay = Duration::from_millis(50);
        let parameters = Parameters {
            leader_timeout,
            min_block_delay,
            ..Default::default()
        };
        let context = Arc::new(context.with_parameters(parameters));
        let start = Instant::now();

        let (mut signals, signal_receivers) = CoreSignals::new(context.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            Arc::new(FakeNetworkClient::default()),
            context.clone(),
            dispatcher.clone(),
            Arc::new(RwLock::new(DagState::new(
                context.clone(),
                Arc::new(MemStore::new(context.clone())),
            ))),
        );

        // spawn the task
        let _handle = LeaderTimeoutTask::start(
            dispatcher.clone(),
            transactions_synchronizer,
            &signal_receivers,
            context.clone(),
        );

        // send a signal that own block was created at round 8 to initialize the
        // broadcaster
        let mut encoder = create_encoder(&context);
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(8, 0, &context, &mut encoder).build(),
        );
        // send a signal that a new round has been advanced to initialize the watcher.
        signals.new_round(9);

        signals
            .new_block(input_block)
            .expect("We should expect correct sending a new block");
        sleep(min_block_delay / 2).await;
        // send a signal that own block was created at round 9, which will start min
        // block delay timeout
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(9, 0, &context, &mut encoder).build(),
        );
        signals
            .new_block(input_block)
            .expect("We should expect correct sending a new block");
        // send a signal that a new round has been advanced.
        signals.new_round(10);

        // wait enough until the min block delay has passed and a new_block call is
        // triggered
        sleep(2 * min_block_delay).await;
        let all_calls = dispatcher.get_new_block_calls().await;
        assert_eq!(all_calls.len(), 1);

        let (round, reason, timestamp) = all_calls[0];
        assert_eq!(round, 10);
        assert_eq!(reason, ReasonToCreateBlock::MinBlockDelayTimeout);
        assert!(
            min_block_delay <= timestamp - start,
            "Leader timeout min setting {:?} should be less than actual time difference {:?}",
            min_block_delay,
            timestamp - start
        );

        // wait enough until a new_block has been received
        sleep(2 * leader_timeout).await;
        let all_calls = dispatcher.get_new_block_calls().await;
        assert_eq!(all_calls.len(), 1);

        let (round, reason, timestamp) = all_calls[0];
        assert_eq!(round, 10);
        assert_eq!(reason, ReasonToCreateBlock::MaxLeaderTimeout);
        assert!(
            leader_timeout <= timestamp - start,
            "Leader timeout setting {:?} should be less than actual time difference {:?}",
            leader_timeout,
            timestamp - start
        );

        // now wait another 2 * leader_timeout, no other call should be received
        sleep(2 * leader_timeout).await;
        let all_calls = dispatcher.get_new_block_calls().await;

        assert_eq!(all_calls.len(), 0);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn multiple_leader_timeouts() {
        telemetry_subscribers::init_for_testing();
        let (context, _signers) = Context::new_for_test(4);
        let dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let leader_timeout = Duration::from_millis(500);
        let min_block_delay = Duration::from_millis(50);
        let parameters = Parameters {
            leader_timeout,
            min_block_delay,
            ..Default::default()
        };
        let context = Arc::new(context.with_parameters(parameters));

        let transactions_synchronizer = TransactionsSynchronizer::start(
            Arc::new(FakeNetworkClient::default()),
            context.clone(),
            dispatcher.clone(),
            Arc::new(RwLock::new(DagState::new(
                context.clone(),
                Arc::new(MemStore::new(context.clone())),
            ))),
        );

        let now = Instant::now();

        let (mut signals, signal_receivers) = CoreSignals::new(context.clone());

        // spawn the task
        let _handle = LeaderTimeoutTask::start(
            dispatcher.clone(),
            transactions_synchronizer,
            &signal_receivers,
            context.clone(),
        );

        // now send some signals with some small delay between them, but not enough so
        // it does not trigger a call of the new block method.
        signals.new_round(13);
        // send a signal that own block was created at round 12
        let mut encoder = create_encoder(&context);

        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(12, 0, &context, &mut encoder).build(),
        );
        signals
            .new_block(input_block)
            .expect("We should expect correct sending a new block");
        sleep(min_block_delay / 2).await;
        signals.new_round(14);
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(13, 0, &context, &mut encoder).build(),
        );
        signals
            .new_block(input_block)
            .expect("We should expect correct sending a new block");
        sleep(min_block_delay / 2).await;

        // Finally signal again and give enough time to trigger block creation
        signals.new_round(15);
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(14, 0, &context, &mut encoder).build(),
        );
        signals
            .new_block(input_block)
            .expect("We should expect correct sending a new block");
        sleep(2 * leader_timeout).await;

        // only the last one should be received
        let all_calls = dispatcher.get_new_block_calls().await;
        let (round, reason, timestamp) = all_calls[0];
        assert_eq!(round, 15);
        assert_eq!(reason, ReasonToCreateBlock::MinBlockDelayTimeout);
        assert!(min_block_delay < timestamp - now);

        let (round, reason, timestamp) = all_calls[1];
        assert_eq!(round, 15);
        assert_eq!(reason, ReasonToCreateBlock::MaxLeaderTimeout);
        assert!(leader_timeout < timestamp - now);
    }
}
