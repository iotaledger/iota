// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, str::FromStr};

use fastcrypto::encoding::{Base64, Encoding};
use iota_sdk_types::ObjectId;
use iota_types::storage::OwnedObjectCursor as IndexCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// The paging cursor of the owner-index queries — `getOwnedObjects`,
/// `getCoins` and `getAllCoins`.
///
/// Opaque on the wire: a caller passes back the `nextCursor` a page returned
/// and reads nothing out of it. It names the position of the row it came from,
/// which is what lets the next page resume after an object that has since been
/// spent.
///
/// Which position it takes depends on how the store answering the query orders
/// its owner index, and the two stores serving this API order it differently:
///
/// - [`Self::ObjectId`] is a position in an index ordered by object id alone,
///   which is what the indexer's is. It is written as the object id, so a
///   cursor the indexer issues reads exactly as it did before this type
///   existed.
/// - [`Self::Position`] is a position in the index a node keeps for its own
///   reads, ordered by object type and balance before the object id. Those
///   fields cannot be recovered from an object id, so the cursor carries them,
///   written as base64.
///
/// A caller hands a cursor back to the endpoint that issued it, so the form
/// always matches the store reading it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedObjectCursor {
    /// A position in an index ordered by object id alone.
    ObjectId(ObjectId),
    /// A position in an index ordered by object type and balance before the
    /// object id.
    Position(IndexCursor),
}

impl OwnedObjectCursor {
    /// A cursor for a store whose owner index is ordered by object id, which
    /// needs nothing else to resume.
    pub fn from_object_id(object_id: ObjectId) -> Self {
        Self::ObjectId(object_id)
    }

    /// A cursor for a store whose owner index is ordered by type and balance.
    pub fn from_position(cursor: IndexCursor) -> Self {
        Self::Position(cursor)
    }

    /// The object the cursor names, which is all a store ordering its owner
    /// index by object id needs.
    pub fn object_id(&self) -> ObjectId {
        match self {
            Self::ObjectId(object_id) => *object_id,
            Self::Position(cursor) => cursor.object_id,
        }
    }

    /// The full position, or `None` for a cursor that names only an object —
    /// one issued by a store that orders its owner index by object id, and so
    /// carries nothing that places a row in an index ordered by anything else.
    pub fn position(&self) -> Option<&IndexCursor> {
        match self {
            Self::ObjectId(_) => None,
            Self::Position(cursor) => Some(cursor),
        }
    }
}

impl fmt::Display for OwnedObjectCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Unchanged from before this type: a cursor of an index ordered by
            // object id is the object id.
            Self::ObjectId(object_id) => write!(f, "{object_id}"),
            Self::Position(cursor) => {
                let bytes = bcs::to_bytes(cursor).map_err(|_| fmt::Error)?;
                write!(f, "{}", Base64::encode(bytes))
            }
        }
    }
}

impl FromStr for OwnedObjectCursor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // An object id is `0x`-prefixed hex and a position is base64, so the
        // two forms cannot be read as one another.
        if let Ok(object_id) = s.parse::<ObjectId>() {
            return Ok(Self::ObjectId(object_id));
        }
        let bytes = Base64::decode(s).map_err(|e| anyhow::anyhow!("invalid cursor: {e}"))?;
        let cursor = bcs::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("invalid cursor: {e}"))?;
        Ok(Self::Position(cursor))
    }
}

impl Serialize for OwnedObjectCursor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for OwnedObjectCursor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        encoded.parse().map_err(D::Error::custom)
    }
}

impl JsonSchema for OwnedObjectCursor {
    fn schema_name() -> String {
        "OwnedObjectCursor".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        // Described rather than left as a bare string: what a caller has to
        // know is that the value is not to be read or built, only handed back.
        let mut schema = String::json_schema(generator).into_object();
        schema.metadata().description = Some(
            "An opaque paging cursor. Pass back the `nextCursor` a page returned to read the \
             page after it. Its contents are not part of the API and an object id is not a \
             valid cursor."
                .to_owned(),
        );
        schema.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A caller only ever hands back what a page gave it, so both forms have
    /// to survive that round trip exactly.
    #[test]
    fn both_forms_survive_the_round_trip_through_the_wire() {
        let by_id = OwnedObjectCursor::from_object_id(ObjectId::random());
        for inverted_balance in [None, Some(0), Some(u64::MAX), Some(42)] {
            let by_position = OwnedObjectCursor::from_position(IndexCursor {
                object_type_identifier: 7,
                object_type_params: 9,
                inverted_balance,
                object_id: ObjectId::random(),
            });
            for cursor in [by_id, by_position] {
                let json = serde_json::to_string(&cursor).unwrap();
                assert!(json.starts_with('"'), "must be a JSON string: {json}");
                assert_eq!(
                    serde_json::from_str::<OwnedObjectCursor>(&json).unwrap(),
                    cursor
                );
            }
        }
    }

    /// The object-id form is written as the object id, so a cursor the indexer
    /// issues reads exactly as it did before this type existed.
    #[test]
    fn the_object_id_form_is_written_as_the_object_id() {
        let object_id = ObjectId::random();
        let cursor = OwnedObjectCursor::from_object_id(object_id);
        assert_eq!(cursor.to_string(), object_id.to_string());
        assert_eq!(
            serde_json::to_string(&cursor).unwrap(),
            serde_json::to_string(&object_id).unwrap()
        );
    }

    /// Both forms answer which object they name; only the one that carries a
    /// position has one.
    #[test]
    fn only_the_position_form_carries_one() {
        let object_id = ObjectId::random();
        let by_id = OwnedObjectCursor::from_object_id(object_id);
        assert_eq!(by_id.object_id(), object_id);
        assert!(by_id.position().is_none());

        let by_position = OwnedObjectCursor::from_position(IndexCursor {
            object_type_identifier: 1,
            object_type_params: 2,
            inverted_balance: Some(3),
            object_id,
        });
        assert_eq!(by_position.object_id(), object_id);
        assert!(by_position.position().is_some());
    }

    #[test]
    fn a_malformed_cursor_is_refused_rather_than_read_as_a_position() {
        assert!("not base64!".parse::<OwnedObjectCursor>().is_err());
        assert!(
            Base64::encode([0xff_u8; 3])
                .parse::<OwnedObjectCursor>()
                .is_err()
        );
    }
}
