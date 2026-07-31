// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_sdk_types::{Address, Event, Identifier, ObjectId, StructTag, TypeTag};
use iota_types::{
    collection_types::VecMap,
    display::{DisplayObject, DisplayVersionUpdatedEvent},
    id::ID,
    object::Object,
};

use crate::{errors::IndexerError, schema::display};

const DISPLAY_STRUCT_NAME: Identifier = Identifier::from_static("Display");

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

    /// Builds a display row from a live `0x2::display::Display<T>` object.
    ///
    /// Returns `None` when the object is not a `Display<T>`.
    pub fn try_from_object(object: &Object) -> Option<Self> {
        let move_object = object.data.as_opt_struct()?;
        let struct_tag = move_object.struct_tag();
        if !is_display(struct_tag) {
            return None;
        }
        let [TypeTag::Struct(object_type)] = struct_tag.type_params() else {
            return None;
        };
        let display: DisplayObject = bcs::from_bytes(move_object.contents()).ok()?;

        Some(Self {
            object_type: object_type.to_canonical_string(/* with_prefix */ true),
            id: display.id.object_id().as_bytes().to_vec(),
            version: display.version as i16,
            bcs: bcs::to_bytes(&display.fields)
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

/// Extracts the canonical name of `T` from the type of a
/// `0x2::display::DisplayCreated<T>` event, without touching the event
/// contents.
///
/// Returns `None` if the provided event is of a different type.
pub fn displayed_type_from_created_event(event: &Event) -> Option<String> {
    if !event.type_.is_display_created() {
        return None;
    }
    let [TypeTag::Struct(displayed_type)] = event.type_.type_params() else {
        return None;
    };
    Some(displayed_type.to_canonical_string(/* with_prefix */ true))
}

/// Extracts the `ObjectId` of the created `Display<T>` object from the
/// contents of a `0x2::display::DisplayCreated<T>` event.
///
/// Returns `None` if the provided event is of a different type.
pub fn display_id_from_created_event(event: &Event) -> Option<ObjectId> {
    if !event.type_.is_display_created() {
        return None;
    }
    let created_event: ID = bcs::from_bytes(&event.contents).ok()?;
    Some(created_event.bytes)
}

/// Returns whether `struct_tag` is of `0x2::display::Display` type.
fn is_display(struct_tag: &StructTag) -> bool {
    struct_tag.address() == Address::FRAMEWORK
        && struct_tag.module() == &Identifier::DISPLAY_MODULE
        && struct_tag.name() == &DISPLAY_STRUCT_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display_event(type_: StructTag, contents: Vec<u8>) -> Event {
        Event {
            package_id: ObjectId::random(),
            module: Identifier::DISPLAY_MODULE,
            sender: Address::random(),
            type_,
            contents,
        }
    }

    /// The type `T` a `Display<T>` refers to.
    fn displayed_type() -> StructTag {
        StructTag::new(
            Address::random(),
            Identifier::from_static("test"),
            Identifier::from_static("Test"),
            vec![],
        )
    }

    #[test]
    fn display_created_event_helpers_extract_type_and_id() {
        let displayed_type = displayed_type();
        let display_id = ObjectId::random();
        let event = display_event(
            StructTag::new_display_created(displayed_type.clone()),
            bcs::to_bytes(&ID::new(display_id)).unwrap(),
        );

        assert_eq!(
            displayed_type_from_created_event(&event),
            Some(displayed_type.to_canonical_string(true))
        );
        assert_eq!(display_id_from_created_event(&event), Some(display_id));
    }

    #[test]
    fn display_created_event_helpers_ignore_other_events() {
        let event = display_event(
            StructTag::new_display_version_updated(displayed_type()),
            vec![],
        );
        assert_eq!(displayed_type_from_created_event(&event), None);
        assert_eq!(display_id_from_created_event(&event), None);
    }
}
