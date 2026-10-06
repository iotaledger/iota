// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{CheckpointSequenceNumber, CheckpointTimestamp, Event, ObjectId};
use iota_types::full_checkpoint_content::CheckpointTransaction;

use crate::{
    ingestion::common::{orchestration::OperationalLevel, prepare::ValidatedCheckpoint},
    models::display::{
        StoredDisplay, display_id_from_created_event, displayed_type_from_created_event,
    },
    types::{EventIndex, IndexedEvent},
};

/// The builder of all event data to commit to the database.
#[derive(Debug)]
pub(crate) struct EventsTransformer<'chk> {
    checkpoint: ValidatedCheckpoint<'chk>,
}

impl<'chk> EventsTransformer<'chk> {
    pub(super) fn new(checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        Self { checkpoint }
    }

    pub(super) fn transform(self, operational_level: OperationalLevel) -> EventData {
        let mut chk_event_data = EventData::new(operational_level);

        for (sequence_number, checkpoint_transaction) in self.checkpoint.enumerate_transactions() {
            let transformer = TransactionEventsTransformer::new(
                checkpoint_transaction,
                sequence_number,
                self.checkpoint.sequence_number(),
                self.checkpoint.timestamp_ms(),
            );
            transformer.extend_event_data(&mut chk_event_data);
        }
        chk_event_data
    }
}

/// The builder of transaction event data to commit to the database.
#[derive(Debug)]
pub(crate) struct TransactionEventsTransformer<'tx> {
    transaction: &'tx CheckpointTransaction,
    tx_sequence_number: u64,
    checkpoint_sequence_number: CheckpointSequenceNumber,
    checkpoint_timestamp_ms: CheckpointTimestamp,
}

impl<'tx> TransactionEventsTransformer<'tx> {
    pub(crate) fn new(
        transaction: &'tx CheckpointTransaction,
        tx_sequence_number: u64,
        checkpoint_sequence_number: CheckpointSequenceNumber,
        checkpoint_timestamp_ms: CheckpointTimestamp,
    ) -> Self {
        Self {
            transaction,
            tx_sequence_number,
            checkpoint_sequence_number,
            checkpoint_timestamp_ms,
        }
    }

    pub(crate) fn transform(self, operational_level: OperationalLevel) -> EventData {
        let mut tx_event_data = EventData::new(operational_level);

        self.extend_event_data(&mut tx_event_data);
        tx_event_data
    }

    /// Adds the event data of the transaction to `data`.
    fn extend_event_data(self, data: &mut EventData) {
        let Some(events) = self.transaction.events.as_ref() else {
            return;
        };

        let mut transaction_displays = BTreeMap::default();
        for (event_sequence_number, chain_event) in events.iter().enumerate() {
            if let Some((display_type, display)) = Self::build_display(chain_event) {
                transaction_displays.insert(display_type, display);
            }
            if let Some(events) = &mut data.events {
                events.push(self.build_event(chain_event, event_sequence_number as u64));
            }
            if let Some(indices) = &mut data.event_indices {
                indices.push(self.build_event_index(chain_event, event_sequence_number as u64));
            }
        }
        // complement any displays created without emitting a DisplayUpdatedEvent
        let display_created_events = events.iter().filter_map(|event| {
            displayed_type_from_created_event(event).map(|display_type| (display_type, event))
        });
        for (display_type, display_created_event) in display_created_events {
            if transaction_displays.contains_key(&display_type) {
                // display is already indexed through a DisplayUpdatedEvent
                continue;
            }
            let Some(display_id) = display_id_from_created_event(display_created_event) else {
                continue;
            };
            if let Some(display) = self.build_display_from_objects(display_id) {
                transaction_displays.insert(display_type, display);
            }
        }
        data.displays.extend(transaction_displays);
    }

    fn build_display(event: &Event) -> Option<(String, StoredDisplay)> {
        StoredDisplay::try_from_event(event).map(|display| (display.object_type.clone(), display))
    }

    fn build_event(&self, event: &Event, event_sequence_number: u64) -> IndexedEvent {
        IndexedEvent::from_event(
            self.tx_sequence_number,
            event_sequence_number,
            self.checkpoint_sequence_number,
            *self.transaction.transaction.digest(),
            event,
            self.checkpoint_timestamp_ms,
        )
    }

    fn build_event_index(&self, event: &Event, event_sequence_number: u64) -> EventIndex {
        EventIndex::from_event(self.tx_sequence_number, event_sequence_number, event)
    }

    fn build_display_from_objects(&self, display_id: ObjectId) -> Option<StoredDisplay> {
        self.transaction
            .output_objects
            .iter()
            .find(|object| object.id() == display_id)
            .and_then(StoredDisplay::try_from_object)
    }
}

#[derive(Debug)]
pub(crate) struct EventData {
    pub(crate) displays: BTreeMap<String, StoredDisplay>,
    pub(crate) events: Option<Vec<IndexedEvent>>,
    pub(crate) event_indices: Option<Vec<EventIndex>>,
}

impl EventData {
    /// Creates empty event data with the collections that the
    /// `operational_level` includes.
    fn new(operational_level: OperationalLevel) -> Self {
        Self {
            displays: Default::default(),
            events: operational_level
                .includes(OperationalLevel::FilteredQueries)
                .then(Default::default),
            event_indices: operational_level
                .includes(OperationalLevel::CombinedEventFilters)
                .then(Default::default),
        }
    }
}
