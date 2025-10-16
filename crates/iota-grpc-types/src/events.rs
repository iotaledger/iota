use iota_json_rpc_types::IotaEvent;

use crate::v0::events::{Event, EventId};

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for Event {
    fn from(event: &IotaEvent) -> Self {
        Event {
            event_id: Some(EventId {
                event_seq: event.id.event_seq,
                tx_digest: Some(crate::v0::common::TransactionDigest {
                    digest: event.id.tx_digest.into_inner().to_vec(),
                }),
            }),
            package_id: Some(crate::v0::common::Address {
                address: event.package_id.to_vec(),
            }),
            transaction_module: event.transaction_module.to_string(),
            sender: Some(crate::v0::common::Address {
                address: event.sender.to_vec(),
            }),
            type_name: event.type_.to_string(),
            parsed_json: event.parsed_json.to_string(),
            timestamp_ms: event.timestamp_ms,
            event_data: Some(crate::v0::common::BcsData {
                data: event.bcs.bytes().to_vec(),
            }),
        }
    }
}
