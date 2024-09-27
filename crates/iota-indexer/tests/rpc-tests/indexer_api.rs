// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::SystemTime};

use iota_json_rpc_api::IndexerApiClient;
use iota_json_rpc_types::{EventFilter, EventPage};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
};
use serial_test::serial;

use crate::common::pg_integration::{
    indexer_wait_for_checkpoint, rpc_call_error_msg_matches,
    start_test_cluster_with_read_write_indexer,
};

#[tokio::test]
#[serial]
async fn query_events_no_events_descending() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let indexer_events = indexer_client
        .query_events(
            EventFilter::Sender(
                IotaAddress::from_str(
                    "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                )
                .unwrap(),
            ),
            None,
            None,
            Some(true),
        )
        .await
        .unwrap();

    assert_eq!(indexer_events, EventPage::empty())
}

#[tokio::test]
#[serial]
async fn query_events_no_events_ascending() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let indexer_events = indexer_client
        .query_events(
            EventFilter::Sender(
                IotaAddress::from_str(
                    "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                )
                .unwrap(),
            ),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(indexer_events, EventPage::empty())
}

#[tokio::test]
#[serial]
async fn query_events_unsupported_events() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    // Get the current time in milliseconds since the UNIX epoch
    let now_millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    // Subtract 10 minutes from the current time
    let ten_minutes_ago = now_millis - (10 * 60 * 1000); // 600 seconds = 10 minutes

    let unsupported_filters = vec![
        (EventFilter::All(vec![]), true),
        (EventFilter::Any(vec![]), true),
        (
            EventFilter::And(
                Box::new(EventFilter::Any(vec![])),
                Box::new(EventFilter::Any(vec![])),
            ),
            true,
        ),
        (
            EventFilter::Or(
                Box::new(EventFilter::Any(vec![])),
                Box::new(EventFilter::Any(vec![])),
            ),
            true,
        ),
        (
            EventFilter::TimeRange {
                start_time: ten_minutes_ago as u64,
                end_time: now_millis as u64,
            },
            true,
        ),
        (
            EventFilter::MoveEventField {
                path: String::default(),
                value: serde_json::Value::Bool(true),
            },
            true,
        ),
        (EventFilter::Sender(IotaAddress::ZERO), false),
        (EventFilter::Transaction(TransactionDigest::ZERO), false),
        (EventFilter::Package(ObjectID::ZERO), false),
        (
            EventFilter::MoveEventModule {
                package: ObjectID::ZERO,
                module: "x".parse().unwrap(),
            },
            false,
        ),
        (
            EventFilter::MoveEventType("0xabcd::MyModule::Foo".parse().unwrap()),
            false,
        ),
        (
            EventFilter::MoveModule {
                package: ObjectID::ZERO,
                module: "x".parse().unwrap(),
            },
            false,
        ),
    ];

    for (event_filter, expected) in unsupported_filters {
        let result = indexer_client
            .query_events(event_filter, None, None, None)
            .await;

        assert_eq!(
            expected,
            rpc_call_error_msg_matches(
                result,
                r#"{"code":-32603,"message": "Indexer does not support the feature with error: `This type of EventFilter is not supported.`"}"#,
            )
        );
    }
}
