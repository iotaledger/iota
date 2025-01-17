// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The IotaSyncer module is responsible for synchronizing Events emitted
//! on IOTA blockchain from concerned modules of bridge package 0x9.

use std::{collections::HashMap, sync::Arc};

use iota_json_rpc_types::IotaEvent;
use iota_metrics::spawn_logged_monitored_task;
use iota_types::{BRIDGE_PACKAGE_ID, Identifier, event::EventID};
use tokio::{
    task::JoinHandle,
    time::{self, Duration},
};

use crate::{
    error::BridgeResult,
    iota_client::{IotaClient, IotaClientInner},
    retry_with_max_elapsed_time,
};

const IOTA_EVENTS_CHANNEL_SIZE: usize = 1000;

/// Map from contract address to their start cursor (exclusive)
pub type IotaTargetModules = HashMap<Identifier, Option<EventID>>;

pub struct IotaSyncer<C> {
    iota_client: Arc<IotaClient<C>>,
    // The last transaction that the syncer has fully processed.
    // Syncer will resume post this transaction (i.e. exclusive), when it starts.
    cursors: IotaTargetModules,
}

impl<C> IotaSyncer<C>
where
    C: IotaClientInner + 'static,
{
    pub fn new(iota_client: Arc<IotaClient<C>>, cursors: IotaTargetModules) -> Self {
        Self {
            iota_client,
            cursors,
        }
    }

    pub async fn run(
        self,
        query_interval: Duration,
    ) -> BridgeResult<(
        Vec<JoinHandle<()>>,
        iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
    )> {
        let (events_tx, events_rx) = iota_metrics::metered_channel::channel(
            IOTA_EVENTS_CHANNEL_SIZE,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["iota_events_queue"]),
        );

        let mut task_handles = vec![];
        for (module, cursor) in self.cursors {
            let events_rx_clone: iota_metrics::metered_channel::Sender<(
                Identifier,
                Vec<IotaEvent>,
            )> = events_tx.clone();
            let iota_client_clone = self.iota_client.clone();
            task_handles.push(spawn_logged_monitored_task!(
                Self::run_event_listening_task(
                    module,
                    cursor,
                    events_rx_clone,
                    iota_client_clone,
                    query_interval
                )
            ));
        }
        Ok((task_handles, events_rx))
    }

    async fn run_event_listening_task(
        // The module where interested events are defined.
        // Module is always of bridge package 0x9.
        module: Identifier,
        mut cursor: Option<EventID>,
        events_sender: iota_metrics::metered_channel::Sender<(Identifier, Vec<IotaEvent>)>,
        iota_client: Arc<IotaClient<C>>,
        query_interval: Duration,
    ) {
        tracing::info!(?module, ?cursor, "Starting iota events listening task");
        let mut interval = time::interval(query_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Ok(Ok(events)) = retry_with_max_elapsed_time!(
                iota_client.query_events_by_module(BRIDGE_PACKAGE_ID, module.clone(), cursor),
                Duration::from_secs(120)
            ) else {
                tracing::error!("Failed to query events from iota client after retry");
                continue;
            };

            let len = events.data.len();
            if len != 0 {
                events_sender
                    .send((module.clone(), events.data))
                    .await
                    .expect("All IOTA event channel receivers are closed");
                if let Some(next) = events.next_cursor {
                    cursor = Some(next);
                }
                tracing::info!(?module, ?cursor, "Observed {len} new IOTA events");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_json_rpc_types::EventPage;
    use iota_types::{Identifier, digests::TransactionDigest, event::EventID};
    use prometheus::Registry;
    use tokio::time::timeout;

    use super::*;
    use crate::{iota_client::IotaClient, iota_mock_client::IotaMockClient};

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    async fn test_iota_syncer_basic() -> anyhow::Result<()> {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        iota_metrics::init_metrics(&registry);

        let mock = IotaMockClient::default();
        let client = Arc::new(IotaClient::new_for_testing(mock.clone()));
        let module_foo = Identifier::new("Foo").unwrap();
        let module_bar = Identifier::new("Bar").unwrap();
        let empty_events = EventPage::empty();
        let cursor = EventID {
            tx_digest: TransactionDigest::random(),
            event_seq: 0,
        };
        add_event_response(&mock, module_foo.clone(), cursor, empty_events.clone());
        add_event_response(&mock, module_bar.clone(), cursor, empty_events.clone());

        let target_modules = HashMap::from_iter(vec![
            (module_foo.clone(), Some(cursor)),
            (module_bar.clone(), Some(cursor)),
        ]);
        let interval = Duration::from_millis(200);
        let (_handles, mut events_rx) = IotaSyncer::new(client, target_modules)
            .run(interval)
            .await
            .unwrap();

        // Initially there are no events
        assert_no_more_events(interval, &mut events_rx).await;

        // Module Foo has new events
        let mut event_1: IotaEvent = IotaEvent::random_for_testing();
        let package_id = BRIDGE_PACKAGE_ID;
        event_1.type_.address = package_id.into();
        event_1.type_.module = module_foo.clone();
        let module_foo_events_1: iota_json_rpc_types::Page<IotaEvent, EventID> = EventPage {
            data: vec![event_1.clone(), event_1.clone()],
            next_cursor: Some(event_1.id),
            has_next_page: false,
        };
        add_event_response(&mock, module_foo.clone(), event_1.id, empty_events.clone());
        add_event_response(
            &mock,
            module_foo.clone(),
            cursor,
            module_foo_events_1.clone(),
        );

        let (identifier, received_events) = events_rx.recv().await.unwrap();
        assert_eq!(identifier, module_foo);
        assert_eq!(received_events.len(), 2);
        assert_eq!(received_events[0].id, event_1.id);
        assert_eq!(received_events[1].id, event_1.id);
        // No more
        assert_no_more_events(interval, &mut events_rx).await;

        // Module Bar has new events
        let mut event_2: IotaEvent = IotaEvent::random_for_testing();
        event_2.type_.address = package_id.into();
        event_2.type_.module = module_bar.clone();
        let module_bar_events_1 = EventPage {
            data: vec![event_2.clone()],
            next_cursor: Some(event_2.id),
            has_next_page: false,
        };
        add_event_response(&mock, module_bar.clone(), event_2.id, empty_events.clone());

        add_event_response(&mock, module_bar.clone(), cursor, module_bar_events_1);

        let (identifier, received_events) = events_rx.recv().await.unwrap();
        assert_eq!(identifier, module_bar);
        assert_eq!(received_events.len(), 1);
        assert_eq!(received_events[0].id, event_2.id);
        // No more
        assert_no_more_events(interval, &mut events_rx).await;

        Ok(())
    }

    async fn assert_no_more_events(
        interval: Duration,
        events_rx: &mut iota_metrics::metered_channel::Receiver<(Identifier, Vec<IotaEvent>)>,
    ) {
        match timeout(interval * 2, events_rx.recv()).await {
            Err(_e) => (),
            other => panic!("Should have timed out, but got: {:?}", other),
        };
    }

    fn add_event_response(
        mock: &IotaMockClient,
        module: Identifier,
        cursor: EventID,
        events: EventPage,
    ) {
        mock.add_event_response(BRIDGE_PACKAGE_ID, module.clone(), cursor, events.clone());
    }
}
