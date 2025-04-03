// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use diesel::prelude::*;
use iota_json_rpc_types::{BcsEvent, IotaEvent, type_and_fields_from_move_event_data};
use iota_package_resolver::{PackageStore, Resolver};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    event::EventID,
    object::bounded_visitor::BoundedVisitor,
    parse_iota_struct_tag,
};
use move_core_types::identifier::Identifier;

use crate::{
    errors::IndexerError,
    schema::{events, optimistic_events},
    types::IndexedEvent,
};

#[derive(Queryable, QueryableByName, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = events)]
pub struct StoredEvent {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub tx_sequence_number: i64,

    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub event_sequence_number: i64,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub transaction_digest: Vec<u8>,

    #[diesel(sql_type = diesel::sql_types::Array<diesel::sql_types::Nullable<diesel::pg::sql_types::Bytea>>)]
    pub senders: Vec<Option<Vec<u8>>>,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub package: Vec<u8>,

    #[diesel(sql_type = diesel::sql_types::Text)]
    pub module: String,

    #[diesel(sql_type = diesel::sql_types::Text)]
    pub event_type: String,

    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub timestamp_ms: i64,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub bcs: Vec<u8>,
}

#[derive(Queryable, QueryableByName, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = optimistic_events)]
pub struct OptimisticEvent {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub tx_insertion_order: i64,

    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub event_sequence_number: i64,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub transaction_digest: Vec<u8>,

    #[diesel(sql_type = diesel::sql_types::Array<diesel::sql_types::Nullable<diesel::pg::sql_types::Bytea>>)]
    pub senders: Vec<Option<Vec<u8>>>,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub package: Vec<u8>,

    #[diesel(sql_type = diesel::sql_types::Text)]
    pub module: String,

    #[diesel(sql_type = diesel::sql_types::Text)]
    pub event_type: String,

    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub bcs: Vec<u8>,
}

pub type SendersType = Vec<Option<Vec<u8>>>;

impl From<IndexedEvent> for StoredEvent {
    fn from(event: IndexedEvent) -> Self {
        Self {
            tx_sequence_number: event.tx_sequence_number as i64,
            event_sequence_number: event.event_sequence_number as i64,
            transaction_digest: event.transaction_digest.into_inner().to_vec(),
            senders: event
                .senders
                .into_iter()
                .map(|sender| Some(sender.to_vec()))
                .collect(),
            package: event.package.to_vec(),
            module: event.module.clone(),
            event_type: event.event_type.clone(),
            bcs: event.bcs.clone(),
            timestamp_ms: event.timestamp_ms as i64,
        }
    }
}

impl From<OptimisticEvent> for StoredEvent {
    fn from(event: OptimisticEvent) -> Self {
        Self {
            tx_sequence_number: event.tx_insertion_order,
            event_sequence_number: event.event_sequence_number,
            transaction_digest: event.transaction_digest,
            senders: event.senders,
            package: event.package,
            module: event.module,
            event_type: event.event_type,
            bcs: event.bcs,
            timestamp_ms: -1,
        }
    }
}

impl From<StoredEvent> for OptimisticEvent {
    fn from(event: StoredEvent) -> Self {
        Self {
            tx_insertion_order: event.tx_sequence_number,
            event_sequence_number: event.event_sequence_number,
            transaction_digest: event.transaction_digest,
            senders: event.senders,
            package: event.package,
            module: event.module,
            event_type: event.event_type,
            bcs: event.bcs,
        }
    }
}

impl StoredEvent {
    pub async fn try_into_iota_event(
        self,
        package_resolver: Arc<Resolver<impl PackageStore>>,
    ) -> Result<IotaEvent, IndexerError> {
        let package_id = ObjectID::from_bytes(self.package.clone()).map_err(|_e| {
            IndexerError::PersistentStorageDataCorruption(format!(
                "Failed to parse event package ID: {:?}",
                self.package
            ))
        })?;
        // Note: IotaEvent only has one sender today, so we always use the first one.
        let sender = {
            {
                self.senders.first().ok_or_else(|| {
                    IndexerError::PersistentStorageDataCorruption(
                        "Event senders should contain at least one address".to_string(),
                    )
                })?
            }
        };
        let sender = match sender {
            Some(ref s) => IotaAddress::from_bytes(s).map_err(|_e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to parse event sender address: {:?}",
                    sender
                ))
            })?,
            None => {
                return Err(IndexerError::PersistentStorageDataCorruption(
                    "Event senders element should not be null".to_string(),
                ));
            }
        };

        let type_ = parse_iota_struct_tag(&self.event_type)?;
        let move_type_layout = package_resolver
            .type_layout(type_.clone().into())
            .await
            .map_err(|e| {
                IndexerError::ResolveMoveStruct(format!(
                    "Failed to convert to iota event with Error: {e}",
                ))
            })?;
        let move_object = BoundedVisitor::deserialize_value(&self.bcs, &move_type_layout)
            .map_err(|e| IndexerError::Serde(e.to_string()))?;
        let (_, parsed_json) = type_and_fields_from_move_event_data(move_object)
            .map_err(|e| IndexerError::Serde(e.to_string()))?;
        let tx_digest =
            TransactionDigest::try_from(self.transaction_digest.as_slice()).map_err(|e| {
                IndexerError::Serde(format!(
                    "Failed to parse transaction digest: {:?}, error: {}",
                    self.transaction_digest, e
                ))
            })?;
        Ok(IotaEvent {
            id: EventID {
                tx_digest,
                event_seq: self.event_sequence_number as u64,
            },
            package_id,
            transaction_module: Identifier::from_str(&self.module)?,
            sender,
            type_,
            bcs: BcsEvent::new(self.bcs),
            parsed_json,
            timestamp_ms: Some(self.timestamp_ms as u64),
        })
    }
}

#[cfg(test)]
mod tests {
    use iota_types::event::Event;
    use move_core_types::{account_address::AccountAddress, language_storage::StructTag};

    use super::*;

    #[test]
    fn test_canonical_string_of_event_type() {
        let tx_digest = TransactionDigest::default();
        let event = Event {
            package_id: ObjectID::random(),
            transaction_module: Identifier::new("test").unwrap(),
            sender: AccountAddress::random().into(),
            type_: StructTag {
                address: AccountAddress::TWO,
                module: Identifier::new("test").unwrap(),
                name: Identifier::new("test").unwrap(),
                type_params: vec![],
            },
            contents: vec![],
        };

        let indexed_event = IndexedEvent::from_event(1, 1, 1, tx_digest, &event, 100);

        let stored_event = StoredEvent::from(indexed_event);

        assert_eq!(
            stored_event.event_type,
            "0x0000000000000000000000000000000000000000000000000000000000000002::test::test"
        );
    }
}
