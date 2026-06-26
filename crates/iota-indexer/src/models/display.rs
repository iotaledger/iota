// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_sdk_types::{Event, TypeTag};
use iota_types::display::DisplayVersionUpdatedEvent;

use crate::schema::display;
use crate::{errors::IndexerError, schema::display};

/// Identifies what type is encoded in [`StoredDisplay`]'s `bcs` column.
#[derive(Debug, Copy, Clone)]
pub enum DisplayBcsKind {
    /// Full [`DisplayVersionUpdatedEvent`] BCS, written from on-chain events.
    Event = 0,
    /// Only the [`DisplayVersionUpdatedEvent::fields`].
    Fields = 1,
}

impl TryFrom<i16> for DisplayBcsKind {
    type Error = IndexerError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Event,
            1 => Self::Fields,
            value => {
                return Err(IndexerError::PersistentStorageDataCorruption(format!(
                    "{value} as DisplayBcsKind"
                )));
            }
        })
    }
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone)]
#[diesel(table_name = display)]
pub struct StoredDisplay {
    pub object_type: String,
    pub id: Vec<u8>,
    pub version: i16,
    pub bcs: Vec<u8>,
    pub bcs_kind: i16,
}

impl StoredDisplay {
    pub fn try_from_event(event: &Event) -> Option<Self> {
        if !event.type_.is_display_version_updated() {
            return None;
        }
        let ty = match event.type_.type_params() {
            [TypeTag::Struct(struct_type)] => struct_type,
            _ => return None,
        };
        let display_event: DisplayVersionUpdatedEvent = bcs::from_bytes(&event.contents).ok()?;

        Some(Self {
            object_type: ty.to_canonical_string(/* with_prefix */ true),
            id: display_event.id.bytes.as_bytes().to_vec(),
            version: display_event.version as i16,
            bcs: bcs::to_bytes(&display_event.fields)
                .expect("serializing a deserialized value should succeed"),
            bcs_kind: DisplayBcsKind::Fields as i16,
        })
    }


    pub fn to_display_fields(&self) -> Result<VecMap<String, String>, IndexerError> {
        match DisplayBcsKind::try_from(self.bcs_kind)? {
            DisplayBcsKind::Event => {
                bcs::from_bytes::<DisplayVersionUpdatedEvent>(&self.bcs).map(|event| event.fields)
            }
            DisplayBcsKind::Fields => bcs::from_bytes(&self.bcs),
        }
        .map_err(|e| {
            IndexerError::PersistentStorageDataCorruption(format!(
                "failed to decode stored display: {e}"
            ))
        })
    }
}
