// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::IotaEvent;

use crate::v0::{common as grpc_common, events as grpc_events};

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for grpc_events::Event {
    fn from(event: &IotaEvent) -> Self {
        grpc_events::Event {
            event_id: Some(grpc_events::EventId {
                event_seq: event.id.event_seq,
                tx_digest: Some(grpc_common::TransactionDigest {
                    digest: event.id.tx_digest.into_inner().to_vec(),
                }),
            }),
            package_id: Some(grpc_common::Address {
                address: event.package_id.to_vec(),
            }),
            transaction_module: event.transaction_module.to_string(),
            sender: Some(grpc_common::Address {
                address: event.sender.to_vec(),
            }),
            type_name: event.type_.to_string(),
            parsed_json: event.parsed_json.to_string(),
            timestamp_ms: event.timestamp_ms,
            event_data: Some(grpc_common::BcsData {
                data: event.bcs.bytes().to_vec(),
            }),
        }
    }
}
