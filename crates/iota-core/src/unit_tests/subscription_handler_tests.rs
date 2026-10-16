// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, mpsc};

use futures::StreamExt;
use iota_json_rpc_types::{EventFilter, IotaEvent, IotaMoveStruct};
use iota_sdk_types::{Identifier, ObjectId, StructTag};
use iota_types::{
    gas_coin::GasCoin, iota_sdk_types_conversions::struct_tag_sdk_to_core,
    object::bounded_visitor::BoundedVisitor,
};
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};
use prometheus_filtered::Registry;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{CHECKPOINT_QUEUE_SIZE, SubscriptionHandler, SubscriptionMetrics};
use crate::streamer::Streamer;

fn event_of(package_id: ObjectId, event_seq: u64) -> IotaEvent {
    let mut event = IotaEvent::random_for_testing();
    event.package_id = package_id;
    event.id.event_seq = event_seq;
    event
}

/// A send to a full queue waits for room rather than dropping the data.
#[tokio::test]
async fn test_a_send_waits_for_a_full_queue() {
    let streamer: Streamer<IotaEvent, IotaEvent, EventFilter> = Streamer::spawn(
        1,
        Arc::new(SubscriptionMetrics::new(&Registry::new())),
        "event",
    );
    let package_id = ObjectId::random();
    let mut events = Box::pin(streamer.subscribe(EventFilter::Package(package_id)));

    let count = 500;
    for event_seq in 0..count {
        streamer.send(event_of(package_id, event_seq)).await;
    }
    for event_seq in 0..count {
        assert_eq!(events.next().await.unwrap().id.event_seq, event_seq);
    }
}

/// Once the subscribers are a full queue of checkpoints behind, every
/// subscription ends rather than missing a checkpoint.
#[tokio::test]
async fn test_a_full_checkpoint_queue_ends_the_subscriptions() {
    let handler = SubscriptionHandler::new(&Registry::new());
    let mut events = Box::pin(handler.subscribe_events(EventFilter::Package(ObjectId::random())));

    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    handler.notify_checkpoint(Box::new(move || {
        started.send(()).unwrap();
        wait_release.recv().unwrap();
        Vec::new()
    }));
    tokio::task::spawn_blocking(move || wait_started.recv().unwrap())
        .await
        .unwrap();

    for _ in 0..CHECKPOINT_QUEUE_SIZE {
        handler.notify_checkpoint(Box::new(Vec::new));
    }
    assert!(handler.has_event_subscribers());
    handler.notify_checkpoint(Box::new(Vec::new));
    // Ended off the notifying thread, which does not wait for it.
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while handler.has_event_subscribers() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the subscriptions must end");

    release.send(()).unwrap();
    assert!(events.next().await.is_none());
}

#[test]
fn test_to_json_value() {
    let move_event = TestEvent {
        creator: AccountAddress::random(),
        name: "test_event".into(),
        data: vec![100, 200, 300],
        coins: vec![
            GasCoin::new(ObjectId::random(), 1000000),
            GasCoin::new(ObjectId::random(), 2000000),
            GasCoin::new(ObjectId::random(), 3000000),
        ],
    };
    let event_bytes = bcs::to_bytes(&move_event).unwrap();
    let iota_move_struct: IotaMoveStruct =
        BoundedVisitor::deserialize_struct(&event_bytes, &TestEvent::layout())
            .unwrap()
            .into();
    let json_value = iota_move_struct.to_json_value();
    assert_eq!(
        Some(&json!("1000000")),
        json_value.pointer("/coins/0/balance")
    );
    assert_eq!(
        Some(&json!("2000000")),
        json_value.pointer("/coins/1/balance")
    );
    assert_eq!(
        Some(&json!("3000000")),
        json_value.pointer("/coins/2/balance")
    );
    assert_eq!(
        Some(&json!(move_event.coins[0].id().to_string())),
        json_value.pointer("/coins/0/id/id")
    );
    assert_eq!(
        Some(&json!(format!("{:#x}", move_event.creator))),
        json_value.pointer("/creator")
    );
    assert_eq!(Some(&json!("100")), json_value.pointer("/data/0"));
    assert_eq!(Some(&json!("200")), json_value.pointer("/data/1"));
    assert_eq!(Some(&json!("300")), json_value.pointer("/data/2"));
    assert_eq!(Some(&json!("test_event")), json_value.pointer("/name"));
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestEvent {
    creator: AccountAddress,
    name: UTF8String,
    data: Vec<u64>,
    coins: Vec<GasCoin>,
}

impl TestEvent {
    fn struct_tag() -> StructTag {
        StructTag::new(
            ObjectId::FRAMEWORK,
            Identifier::from_static("IOTA"),
            Identifier::from_static("new_foobar"),
            vec![],
        )
    }

    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::struct_tag()),
            fields: vec![
                MoveFieldLayout::new(ident_str!("creator").to_owned(), MoveTypeLayout::Address),
                MoveFieldLayout::new(
                    ident_str!("name").to_owned(),
                    MoveTypeLayout::Struct(Box::new(UTF8String::layout())),
                ),
                MoveFieldLayout::new(
                    ident_str!("data").to_owned(),
                    MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U64)),
                ),
                MoveFieldLayout::new(
                    ident_str!("coins").to_owned(),
                    MoveTypeLayout::Vector(Box::new(MoveTypeLayout::Struct(Box::new(
                        GasCoin::layout(),
                    )))),
                ),
            ],
        }
    }
}

// Rust version of the Move std::string::String type
// TODO: Do we need this in the iota-types lib?
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct UTF8String {
    bytes: String,
}

impl From<&str> for UTF8String {
    fn from(s: &str) -> Self {
        Self {
            bytes: s.to_string(),
        }
    }
}

impl UTF8String {
    fn struct_tag() -> StructTag {
        StructTag::new_string()
    }
    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::struct_tag()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("bytes").to_owned(),
                MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),
            )],
        }
    }
}
