// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `BridgeOrchestrator` is the component that:
//! 1. monitors IOTA and Ethereum events with the help of `IotaSyncer` and
//!    `EthSyncer`
//! 2. updates WAL table and cursor tables
//! 2. hands actions to `BridgeExecutor` for execution

use std::sync::Arc;

use ethers::types::Address as EthAddress;
use iota_json_rpc_types::IotaEvent;
use iota_metrics::spawn_logged_monitored_task;
use iota_types::Identifier;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{
    abi::EthBridgeEvent,
    action_executor::{
        BridgeActionExecutionWrapper, BridgeActionExecutorTrait, submit_to_executor,
    },
    error::BridgeError,
    events::IotaBridgeEvent,
    iota_client::{IotaClient, IotaClientInner},
    metrics::BridgeMetrics,
    storage::BridgeOrchestratorTables,
    types::EthLog,
};

pub struct BridgeOrchestrator<C> {
    _iota_client: Arc<IotaClient<C>>,
    iota_events_rx: iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
    eth_events_rx: iota_metrics::metered_channel::Receiver<(EthAddress, u64, Vec<EthLog>)>,
    store: Arc<BridgeOrchestratorTables>,
    monitor_tx: iota_metrics::metered_channel::Sender<IotaBridgeEvent>,
    metrics: Arc<BridgeMetrics>,
}

impl<C> BridgeOrchestrator<C>
where
    C: IotaClientInner + 'static,
{
    pub fn new(
        iota_client: Arc<IotaClient<C>>,
        iota_events_rx: iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
        eth_events_rx: iota_metrics::metered_channel::Receiver<(EthAddress, u64, Vec<EthLog>)>,
        store: Arc<BridgeOrchestratorTables>,
        monitor_tx: iota_metrics::metered_channel::Sender<IotaBridgeEvent>,
        metrics: Arc<BridgeMetrics>,
    ) -> Self {
        Self {
            _iota_client: iota_client,
            iota_events_rx,
            eth_events_rx,
            store,
            monitor_tx,
            metrics,
        }
    }

    pub async fn run(
        self,
        bridge_action_executor: impl BridgeActionExecutorTrait,
    ) -> Vec<JoinHandle<()>> {
        tracing::info!("Starting BridgeOrchestrator");
        let mut task_handles = vec![];
        let store_clone = self.store.clone();

        // Spawn BridgeActionExecutor
        let (handles, executor_sender) = bridge_action_executor.run();
        task_handles.extend(handles);
        let executor_sender_clone = executor_sender.clone();
        let metrics_clone = self.metrics.clone();
        task_handles.push(spawn_logged_monitored_task!(Self::run_iota_watcher(
            store_clone,
            executor_sender_clone,
            self.iota_events_rx,
            self.monitor_tx,
            metrics_clone,
        )));
        let store_clone = self.store.clone();

        // Re-submit pending actions to executor
        let actions = store_clone
            .get_all_pending_actions()
            .into_values()
            .collect::<Vec<_>>();
        for action in actions {
            submit_to_executor(&executor_sender, action)
                .await
                .expect("Submit to executor should not fail");
        }

        let metrics_clone = self.metrics.clone();
        task_handles.push(spawn_logged_monitored_task!(Self::run_eth_watcher(
            store_clone,
            executor_sender,
            self.eth_events_rx,
            metrics_clone,
        )));

        task_handles
    }

    async fn run_iota_watcher(
        store: Arc<BridgeOrchestratorTables>,
        executor_tx: iota_metrics::metered_channel::Sender<BridgeActionExecutionWrapper>,
        mut iota_events_rx: iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
        monitor_tx: iota_metrics::metered_channel::Sender<IotaBridgeEvent>,
        metrics: Arc<BridgeMetrics>,
    ) {
        info!("Starting iota watcher task");
        while let Some((identifier, events)) = iota_events_rx.recv().await {
            if events.is_empty() {
                continue;
            }
            info!("Received {} IOTA events: {:?}", events.len(), events);
            metrics
                .iota_watcher_received_events
                .inc_by(events.len() as u64);
            let bridge_events = events
                .iter()
                .filter_map(|iota_event| {
                    match IotaBridgeEvent::try_from_iota_event(iota_event) {
                        Ok(bridge_event) => Some(bridge_event),
                        // On testnet some early bridge transactions could have zero value (before
                        // we disallow it in Move)
                        Err(BridgeError::ZeroValueBridgeTransfer(_)) => {
                            error!("Zero value bridge transfer: {:?}", iota_event);
                            None
                        }
                        Err(e) => {
                            panic!(
                                "IOTA Event could not be deserialized to IotaBridgeEvent: {:?}",
                                e
                            );
                        }
                    }
                })
                .collect::<Vec<_>>();

            let mut actions = vec![];
            for (iota_event, opt_bridge_event) in events.iter().zip(bridge_events) {
                if opt_bridge_event.is_none() {
                    // TODO: we probably should not miss any events, log for now.
                    metrics.iota_watcher_unrecognized_events.inc();
                    error!("IOTA event not recognized: {:?}", iota_event);
                    continue;
                }
                // Unwrap safe: checked above
                let bridge_event: IotaBridgeEvent = opt_bridge_event.unwrap();
                info!("Observed IOTA bridge event: {:?}", bridge_event);

                // Send event to monitor
                monitor_tx
                    .send(bridge_event.clone())
                    .await
                    .expect("Sending event to monitor channel should not fail");

                if let Some(action) = bridge_event
                    .try_into_bridge_action(iota_event.id.tx_digest, iota_event.id.event_seq as u16)
                {
                    actions.push(action);
                }
            }

            if !actions.is_empty() {
                info!(
                    "Received {} actions from IOTA: {:?}",
                    actions.len(),
                    actions
                );
                metrics
                    .iota_watcher_received_actions
                    .inc_by(actions.len() as u64);
                // Write action to pending WAL
                store
                    .insert_pending_actions(&actions)
                    .expect("Store operation should not fail");
                for action in actions {
                    submit_to_executor(&executor_tx, action)
                        .await
                        .expect("Submit to executor should not fail");
                }
            }

            // Unwrap safe: in the beginning of the loop we checked that events is not empty
            let cursor = events.last().unwrap().id;
            store
                .update_iota_event_cursor(identifier, cursor)
                .expect("Store operation should not fail");
        }
        panic!("IOTA event channel was closed unexpectedly");
    }

    async fn run_eth_watcher(
        store: Arc<BridgeOrchestratorTables>,
        executor_tx: iota_metrics::metered_channel::Sender<BridgeActionExecutionWrapper>,
        mut eth_events_rx: iota_metrics::metered_channel::Receiver<(
            ethers::types::Address,
            u64,
            Vec<EthLog>,
        )>,
        metrics: Arc<BridgeMetrics>,
    ) {
        info!("Starting eth watcher task");
        while let Some((contract, end_block, logs)) = eth_events_rx.recv().await {
            if logs.is_empty() {
                store
                    .update_eth_event_cursor(contract, end_block)
                    .expect("Store operation should not fail");
                continue;
            }

            info!("Received {} Eth events", logs.len());
            metrics
                .eth_watcher_received_events
                .inc_by(logs.len() as u64);

            let bridge_events = logs
                .iter()
                .map(EthBridgeEvent::try_from_eth_log)
                .collect::<Vec<_>>();

            let mut actions = vec![];
            for (log, opt_bridge_event) in logs.iter().zip(bridge_events) {
                if opt_bridge_event.is_none() {
                    // TODO: we probably should not miss any events, log for now.
                    metrics.eth_watcher_unrecognized_events.inc();
                    error!("Eth event not recognized: {:?}", log);
                    continue;
                }
                // Unwrap safe: checked above
                let bridge_event = opt_bridge_event.unwrap();
                info!("Observed Eth bridge event: {:?}", bridge_event);

                match bridge_event.try_into_bridge_action(log.tx_hash, log.log_index_in_tx) {
                    Ok(Some(action)) => actions.push(action),
                    Ok(None) => {}
                    Err(e) => {
                        error!(eth_tx_hash=?log.tx_hash, eth_event_index=?log.log_index_in_tx, "Error converting EthBridgeEvent to BridgeAction: {:?}", e);
                    }
                }
                // TODO: handle non Action events
            }
            if !actions.is_empty() {
                info!("Received {} actions from Eth: {:?}", actions.len(), actions);
                metrics
                    .eth_watcher_received_actions
                    .inc_by(actions.len() as u64);
                // Write action to pending WAL
                store
                    .insert_pending_actions(&actions)
                    .expect("Store operation should not fail");
                // Execution will remove the pending actions from DB when the action is
                // completed.
                for action in actions {
                    submit_to_executor(&executor_tx, action)
                        .await
                        .expect("Submit to executor should not fail");
                }
            }

            store
                .update_eth_event_cursor(contract, end_block)
                .expect("Store operation should not fail");
        }
        panic!("Eth event channel was closed");
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ethers::types::{Address as EthAddress, TxHash};
    use prometheus::Registry;

    use super::*;
    use crate::{
        events::{init_all_struct_tags, tests::get_test_iota_event_and_action},
        iota_mock_client::IotaMockClient,
        test_utils::{
            get_test_eth_to_iota_bridge_action, get_test_iota_to_eth_bridge_action,
            get_test_log_and_action,
        },
        types::BridgeActionDigest,
    };

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    async fn test_iota_watcher_task() {
        // Note: this test may fail because of the following reasons:
        // the IotaEvent's struct tag does not match the ones in events.rs

        let (
            iota_events_tx,
            iota_events_rx,
            _eth_events_tx,
            eth_events_rx,
            monitor_tx,
            _monitor_rx,
            iota_client,
            store,
        ) = setup();
        let (executor, mut executor_requested_action_rx) = MockExecutor::new();
        // start orchestrator
        let registry = Registry::new();
        let metrics = Arc::new(BridgeMetrics::new(&registry));
        let _handles = BridgeOrchestrator::new(
            Arc::new(iota_client),
            iota_events_rx,
            eth_events_rx,
            store.clone(),
            monitor_tx,
            metrics,
        )
        .run(executor)
        .await;

        let identifier = Identifier::from_str("test_iota_watcher_task").unwrap();
        let (iota_event, bridge_action) = get_test_iota_event_and_action(identifier.clone());
        iota_events_tx
            .send((identifier.clone(), vec![iota_event.clone()]))
            .await
            .unwrap();

        let start = std::time::Instant::now();
        // Executor should have received the action
        assert_eq!(
            executor_requested_action_rx.recv().await.unwrap(),
            bridge_action.digest()
        );
        loop {
            let actions = store.get_all_pending_actions();
            if actions.is_empty() {
                if start.elapsed().as_secs() > 5 {
                    panic!("Timed out waiting for action to be written to WAL");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                continue;
            }
            assert_eq!(actions.len(), 1);
            let action = actions.get(&bridge_action.digest()).unwrap();
            assert_eq!(action, &bridge_action);
            assert_eq!(
                store.get_iota_event_cursors(&[identifier]).unwrap()[0].unwrap(),
                iota_event.id,
            );
            break;
        }
    }

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    async fn test_eth_watcher_task() {
        // Note: this test may fail because of the following reasons:
        // 1. Log and BridgeAction returned from `get_test_log_and_action` are not in
        //    sync
        // 2. Log returned from `get_test_log_and_action` is not parseable log (not
        //    abigen!, check abi.rs)

        let (
            _iota_events_tx,
            iota_events_rx,
            eth_events_tx,
            eth_events_rx,
            monitor_tx,
            _monitor_rx,
            iota_client,
            store,
        ) = setup();
        let (executor, mut executor_requested_action_rx) = MockExecutor::new();
        // start orchestrator
        let registry = Registry::new();
        let metrics = Arc::new(BridgeMetrics::new(&registry));
        let _handles = BridgeOrchestrator::new(
            Arc::new(iota_client),
            iota_events_rx,
            eth_events_rx,
            store.clone(),
            monitor_tx,
            metrics,
        )
        .run(executor)
        .await;
        let address = EthAddress::random();
        let (log, bridge_action) = get_test_log_and_action(address, TxHash::random(), 10);
        let log_index_in_tx = 10;
        let log_block_num = log.block_number.unwrap().as_u64();
        let eth_log = EthLog {
            log: log.clone(),
            tx_hash: log.transaction_hash.unwrap(),
            block_number: log_block_num,
            log_index_in_tx,
        };
        let end_block_num = log_block_num + 15;

        eth_events_tx
            .send((address, end_block_num, vec![eth_log.clone()]))
            .await
            .unwrap();

        // Executor should have received the action
        assert_eq!(
            executor_requested_action_rx.recv().await.unwrap(),
            bridge_action.digest()
        );
        let start = std::time::Instant::now();
        loop {
            let actions = store.get_all_pending_actions();
            if actions.is_empty() {
                if start.elapsed().as_secs() > 5 {
                    panic!("Timed out waiting for action to be written to WAL");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                continue;
            }
            assert_eq!(actions.len(), 1);
            let action = actions.get(&bridge_action.digest()).unwrap();
            assert_eq!(action, &bridge_action);
            assert_eq!(
                store.get_eth_event_cursors(&[address]).unwrap()[0].unwrap(),
                end_block_num,
            );
            break;
        }
    }

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    /// Test that when orchestrator starts, all pending actions are sent to
    /// executor
    async fn test_resume_actions_in_pending_logs() {
        let (
            _iota_events_tx,
            iota_events_rx,
            _eth_events_tx,
            eth_events_rx,
            monitor_tx,
            _monitor_rx,
            iota_client,
            store,
        ) = setup();
        let (executor, mut executor_requested_action_rx) = MockExecutor::new();

        let action1 = get_test_iota_to_eth_bridge_action(
            None,
            Some(0),
            Some(99),
            Some(10000),
            None,
            None,
            None,
        );

        let action2 = get_test_eth_to_iota_bridge_action(None, None, None, None);
        store
            .insert_pending_actions(&vec![action1.clone(), action2.clone()])
            .unwrap();

        // start orchestrator
        let registry = Registry::new();
        let metrics = Arc::new(BridgeMetrics::new(&registry));
        let _handles = BridgeOrchestrator::new(
            Arc::new(iota_client),
            iota_events_rx,
            eth_events_rx,
            store.clone(),
            monitor_tx,
            metrics,
        )
        .run(executor)
        .await;

        // Executor should have received the action
        let mut digests = std::collections::HashSet::new();
        digests.insert(executor_requested_action_rx.recv().await.unwrap());
        digests.insert(executor_requested_action_rx.recv().await.unwrap());
        assert!(digests.contains(&action1.digest()));
        assert!(digests.contains(&action2.digest()));
        assert_eq!(digests.len(), 2);
    }

    #[expect(clippy::type_complexity)]
    fn setup() -> (
        iota_metrics::metered_channel::Sender<(Identifier, Vec<IotaEvent>)>,
        iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
        iota_metrics::metered_channel::Sender<(EthAddress, u64, Vec<EthLog>)>,
        iota_metrics::metered_channel::Receiver<(EthAddress, u64, Vec<EthLog>)>,
        iota_metrics::metered_channel::Sender<IotaBridgeEvent>,
        iota_metrics::metered_channel::Receiver<IotaBridgeEvent>,
        IotaClient<IotaMockClient>,
        Arc<BridgeOrchestratorTables>,
    ) {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        iota_metrics::init_metrics(&registry);

        init_all_struct_tags();

        let temp_dir = tempfile::tempdir().unwrap();
        let store = BridgeOrchestratorTables::new(temp_dir.path());

        let mock_client = IotaMockClient::default();
        let iota_client = IotaClient::new_for_testing(mock_client.clone());

        let (eth_events_tx, eth_events_rx) = iota_metrics::metered_channel::channel(
            100,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["unit_test_eth_events_queue"]),
        );

        let (iota_events_tx, iota_events_rx) = iota_metrics::metered_channel::channel(
            100,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["unit_test_iota_events_queue"]),
        );
        let (monitor_tx, monitor_rx) = iota_metrics::metered_channel::channel(
            10000,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["monitor_queue"]),
        );
        (
            iota_events_tx,
            iota_events_rx,
            eth_events_tx,
            eth_events_rx,
            monitor_tx,
            monitor_rx,
            iota_client,
            store,
        )
    }

    /// A `BridgeActionExecutorTrait` implementation that only tracks the
    /// submitted actions.
    struct MockExecutor {
        requested_transactions_tx: tokio::sync::broadcast::Sender<BridgeActionDigest>,
    }

    impl MockExecutor {
        fn new() -> (Self, tokio::sync::broadcast::Receiver<BridgeActionDigest>) {
            let (tx, rx) = tokio::sync::broadcast::channel(100);
            (
                Self {
                    requested_transactions_tx: tx,
                },
                rx,
            )
        }
    }

    impl BridgeActionExecutorTrait for MockExecutor {
        fn run(
            self,
        ) -> (
            Vec<tokio::task::JoinHandle<()>>,
            iota_metrics::metered_channel::Sender<BridgeActionExecutionWrapper>,
        ) {
            let (tx, mut rx) = iota_metrics::metered_channel::channel::<BridgeActionExecutionWrapper>(
                100,
                &iota_metrics::get_metrics()
                    .unwrap()
                    .channel_inflight
                    .with_label_values(&["unit_test_mock_executor"]),
            );

            let handles = tokio::spawn(async move {
                while let Some(action) = rx.recv().await {
                    self.requested_transactions_tx
                        .send(action.0.digest())
                        .unwrap();
                }
            });
            (vec![handles], tx)
        }
    }
}
