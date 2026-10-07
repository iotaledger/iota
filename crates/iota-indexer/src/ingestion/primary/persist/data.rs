// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use crate::{
    ingestion::common::prepare::CheckpointObjectChanges,
    models::{
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate},
        obj_indices::StoredObjectVersion,
        objects::StoredBackwardHistoryObject,
    },
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEvent, IndexedObject,
        IndexedPackage, IndexedTransaction, TxIndex,
    },
};

#[derive(Debug, Default)]
pub(crate) struct CheckpointBatch {
    pub(crate) basic: Vec<Basic>,
    pub(crate) objects_history: Option<Vec<ObjectsHistory>>,
    pub(crate) filtered_queries: Option<Vec<FilteredQueries>>,
    pub(crate) combined_event_filters: Option<Vec<CombinedEventFilters>>,
}

impl CheckpointBatch {
    pub(crate) fn len(&self) -> usize {
        self.basic.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.basic.is_empty()
    }

    pub(crate) fn push(&mut self, data: CheckpointDataToCommit) {
        let CheckpointDataToCommit {
            basic,
            objects_history,
            filtered_queries,
            combined_event_filters,
        } = data;
        self.basic.push(basic);
        if let Some(objects_history) = objects_history {
            self.objects_history
                .get_or_insert_default()
                .push(objects_history);
        }
        if let Some(filtered_queries) = filtered_queries {
            self.filtered_queries
                .get_or_insert_default()
                .push(filtered_queries);
        }
        if let Some(combined_event_filters) = combined_event_filters {
            self.combined_event_filters
                .get_or_insert_default()
                .push(combined_event_filters);
        }
    }
}

#[derive(Debug)]
pub(crate) struct CheckpointDataToCommit {
    pub(crate) basic: Basic,
    pub(crate) objects_history: Option<ObjectsHistory>,
    pub(crate) filtered_queries: Option<FilteredQueries>,
    pub(crate) combined_event_filters: Option<CombinedEventFilters>,
}

#[derive(Debug)]
pub(crate) struct Basic {
    pub(crate) checkpoint: IndexedCheckpoint,
    pub(crate) transactions: Vec<IndexedTransaction>,
    pub(crate) packages: Vec<IndexedPackage>,
    pub(crate) object_changes: CheckpointObjectChanges,
    pub(crate) object_versions: Vec<StoredObjectVersion>,
    pub(crate) displays: BTreeMap<String, StoredDisplay>,
    pub(crate) epoch: Option<EpochToCommit>,
}

#[derive(Debug)]
pub(crate) struct ObjectsHistory {
    pub(crate) history_objects: Vec<StoredBackwardHistoryObject>,
}

#[derive(Debug, Default)]
pub(crate) struct FilteredQueries {
    pub(crate) tx_indices: Vec<TxIndex>,
    pub(crate) events: Vec<IndexedEvent>,
}

#[derive(Debug)]
pub(crate) struct CombinedEventFilters {
    pub(crate) event_indices: Vec<EventIndex>,
}

#[derive(Clone, Debug, Default)]
pub struct TransactionObjectChangesToCommit {
    pub changed_objects: Vec<IndexedObject>,
    pub deleted_objects: Vec<IndexedDeletedObject>,
}

#[derive(Clone, Debug)]
pub struct EpochToCommit {
    pub(crate) last_epoch: Option<EndOfEpochUpdate>,
    pub(crate) new_epoch: StartOfEpochUpdate,
}
