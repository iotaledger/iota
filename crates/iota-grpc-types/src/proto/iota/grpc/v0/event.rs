// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");
include!("../../../generated/iota.grpc.v0.event.accessors.rs");

use iota_json_rpc_types::IotaEvent;

use crate::v0::{bcs as grpc_bcs, event as grpc_event, types as grpc_types};

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for grpc_event::Event {
    fn from(event: &IotaEvent) -> Self {
        grpc_event::Event {
            bcs: Some(grpc_bcs::BcsData {
                data: bcs::to_bytes(&event)
                    .expect("BCS serialization should not fail")
                    .into(),
            }),
            package_id: Some(grpc_types::Address {
                address: event.package_id.into_bytes().to_vec().into(),
            }),
            module: Some(event.transaction_module.to_string()),
            sender: Some(grpc_types::Address {
                address: event.sender.to_vec().into(),
            }),
            event_type: Some(event.type_.to_string()),
            bcs_contents: Some(grpc_bcs::BcsData {
                data: event.bcs.clone().into_bytes().into(),
            }),
            json_contents: None, // TODO: fill in JSON contents
        }
    }
}
