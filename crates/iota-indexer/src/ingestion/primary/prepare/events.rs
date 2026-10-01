// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{CheckpointSequenceNumber, CheckpointTimestamp, Event, ObjectId};
use iota_types::full_checkpoint_content::CheckpointTransaction;

use crate::{
    models::display::{
        StoredDisplay, display_id_from_created_event, displayed_type_from_created_event,
    },
    types::{EventIndex, IndexedEvent},
};

/// The builder of all event data to commit to the database.
#[derive(Debug)]
pub(crate) struct EventsTransformer<'tx> {
    transaction: &'tx CheckpointTransaction,
    tx_sequence_number: u64,
    checkpoint_sequence_number: CheckpointSequenceNumber,
    checkpoint_timestamp_ms: CheckpointTimestamp,
}

impl<'tx> EventsTransformer<'tx> {
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

    pub(crate) fn transform(self) -> EventData {
        let mut derived_data = EventData::default();
        let Some(events) = self.transaction.events.as_ref() else {
            return derived_data;
        };
        for (event_sequence_number, chain_event) in events.iter().enumerate() {
            if let Some((display_type, display)) = Self::build_display(chain_event) {
                derived_data.displays.insert(display_type, display);
            }
            let event = self.build_event(chain_event, event_sequence_number as u64);
            derived_data.events.push(event);
            let event_index = self.build_event_index(chain_event, event_sequence_number as u64);
            derived_data.event_indices.push(event_index);
        }
        // complement any displays created without emitting a DisplayUpdatedEvent
        let display_created_events = events.iter().filter_map(|event| {
            displayed_type_from_created_event(event).map(|display_type| (display_type, event))
        });
        for (display_type, display_created_event) in display_created_events {
            if derived_data.displays.contains_key(&display_type) {
                // display is already indexed through a DisplayUpdatedEvent
                continue;
            }
            let Some(display_id) = display_id_from_created_event(display_created_event) else {
                continue;
            };
            if let Some(display) = self.build_display_from_objects(display_id) {
                derived_data.displays.insert(display_type, display);
            }
        }
        derived_data
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

#[derive(Debug, Default)]
pub(crate) struct EventData {
    pub(crate) displays: BTreeMap<String, StoredDisplay>,
    pub(crate) events: Vec<IndexedEvent>,
    pub(crate) event_indices: Vec<EventIndex>,
}
