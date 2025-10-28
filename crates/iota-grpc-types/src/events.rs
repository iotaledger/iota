// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::IotaEvent;

use crate::v0::common as grpc_common;

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for grpc_common::Event {
    fn from(event: &IotaEvent) -> Self {
        grpc_common::Event {
            event_id: Some(grpc_common::EventId {
                event_seq: event.id.event_seq,
                tx_digest: Some(grpc_common::Digest {
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
            timestamp_ms: event.timestamp_ms,
            event_data: Some(grpc_common::Bcs {
                name: event.type_.to_string(),
                value: Some(grpc_common::BcsData {
                    data: event.bcs.bytes().to_vec(),
                }),
            }),
        }
    }
}
