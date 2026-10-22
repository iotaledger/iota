// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The deny-list machine. Reads a dynamic field of the coin deny list as it
//! stood once every commit up to the horizon had executed, for the coin deny
//! check at the commit. It answers live or absent, not keep, drop or missing.
//!
//! The read walks down from the id's highest handler row to the highest one
//! at or below the horizon, so it relies on the field's versions rising with
//! the commits that change it. That holds for the deny list's fields, which
//! change only through the shared deny list object, in the order of the
//! commits that change it. When every row is above the horizon, the field is
//! as the epoch started: the version the lowest row replaced, or absent if
//! that row created it. An id with no row reads the store and then the rows
//! again, for the reason the module doc gives for the owned and shared
//! machines.

use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    error::{IotaError, IotaResult},
    object::Object,
    storage::ObjectKey,
};

use super::{DenyListField, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind,
};

// ---------------------------------------------------------------------------
// Carrier
// ---------------------------------------------------------------------------

/// Marker for the deny-list machine's states.
pub trait DenyListFieldState {}

/// One deny-list field being read at commit `C`. `S` is what the reader has
/// established so far. Each state exposes only the transition the algorithm
/// allows next, and every transition consumes the reader.
pub struct DenyListFieldReader<S: DenyListFieldState> {
    id: ObjectId,
    /// The commit being validated minus the protocol config's horizon
    /// distance. Rows produced above it are walked past.
    horizon: CommitIndex,
    state: S,
}

impl<S: DenyListFieldState> DenyListFieldReader<S> {
    fn into_state<T: DenyListFieldState>(self, state: T) -> DenyListFieldReader<T> {
        DenyListFieldReader {
            id: self.id,
            horizon: self.horizon,
            state,
        }
    }

    /// The next step from `row`, a handler row of `id`.
    fn decide_row(self, (key, row): (ObjectKey, HandlerProcessedObject)) -> RowLookup {
        match classify_row(&row, self.horizon) {
            RowClass::Live => RowLookup::NeedBytes(self.into_state(NeedBytes { version: key.1 })),
            RowClass::Absent => RowLookup::Decided(DenyListField::Absent),
            RowClass::AboveHorizon => RowLookup::AboveHorizon(self.into_state(AboveHorizon {
                version: key.1,
                replaced_version: row.replaced_version,
            })),
        }
    }
}

/// Outcome of a row read.
#[must_use]
pub enum RowLookup {
    /// A live version at the horizon. Next: its bytes.
    NeedBytes(DenyListFieldReader<NeedBytes>),
    /// Decided without bytes.
    Decided(DenyListField),
    /// A row above the horizon. Next: the row below it.
    AboveHorizon(DenyListFieldReader<AboveHorizon>),
}

// ---------------------------------------------------------------------------
// Rows: the highest row of the id
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl DenyListFieldState for Start {}

/// Outcome of the first read of the id's highest row.
#[must_use]
pub enum HighestRowLookup {
    Row(RowLookup),
    /// Not a decision. No row, so the machine continues with the store.
    NoRow(DenyListFieldReader<NoRow>),
}

impl DenyListFieldReader<Start> {
    /// Begins the read of deny-list field `id` at `horizon`. The only
    /// constructor of a reader in any state.
    pub fn start(id: ObjectId, horizon: CommitIndex) -> DenyListFieldReader<Start> {
        DenyListFieldReader {
            id,
            horizon,
            state: Start,
        }
    }

    /// The id's highest row, from the overlay or the durable table.
    pub fn read_highest_row(self, ctx: &CommitIndexedReader) -> IotaResult<HighestRowLookup> {
        Ok(
            match ctx.epoch_store.highest_handler_processed_row(&self.id)? {
                Some(row) => HighestRowLookup::Row(self.decide_row(row)),
                None => HighestRowLookup::NoRow(self.into_state(NoRow)),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Walk: the row below one above the horizon
// ---------------------------------------------------------------------------

/// A row above the horizon, at `version`.
pub struct AboveHorizon {
    version: Version,
    /// The version that row's write replaced, `None` for a creation.
    replaced_version: Option<Version>,
}
impl DenyListFieldState for AboveHorizon {}

impl DenyListFieldReader<AboveHorizon> {
    /// The id's highest row below the current one. With none, the current
    /// row is the epoch's first write of the field, so the field is as the
    /// epoch started: the version that row replaced, or absent if it created
    /// the field.
    pub fn read_row_below(self, ctx: &CommitIndexedReader) -> IotaResult<RowLookup> {
        Ok(
            match ctx
                .epoch_store
                .highest_handler_processed_row_below(&self.id, self.state.version)?
            {
                Some(row) => self.decide_row(row),
                None => match self.state.replaced_version {
                    Some(version) => RowLookup::NeedBytes(self.into_state(NeedBytes { version })),
                    None => RowLookup::Decided(DenyListField::Absent),
                },
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Store: the latest object, then the rows again
// ---------------------------------------------------------------------------

/// No row for the id. The store has not been asked.
pub struct NoRow;
impl DenyListFieldState for NoRow {}

impl DenyListFieldReader<NoRow> {
    /// The latest object by id, or `None` for a tombstone or an unknown id.
    /// Held, not decided: the rows must be read again first.
    pub fn read_object(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<DenyListFieldReader<ObjectAnswered>> {
        let object = ctx.cache.try_get_object(&self.id)?;
        Ok(self.into_state(ObjectAnswered { object }))
    }
}

/// The store answered with the latest object or nothing. Held until the rows
/// are read again.
pub struct ObjectAnswered {
    object: Option<Object>,
}
impl DenyListFieldState for ObjectAnswered {}

impl DenyListFieldReader<ObjectAnswered> {
    /// Reads the highest row again. A row decides as on the first pass. Else
    /// the held object is epoch-start state and stands.
    pub fn reread_highest_row(self, ctx: &CommitIndexedReader) -> IotaResult<RowLookup> {
        if let Some(row) = ctx.epoch_store.highest_handler_processed_row(&self.id)? {
            return Ok(self.decide_row(row));
        }
        Ok(RowLookup::Decided(match self.state.object {
            Some(object) => DenyListField::Live(object),
            None => DenyListField::Absent,
        }))
    }
}

// ---------------------------------------------------------------------------
// Bytes: the version live at the horizon
// ---------------------------------------------------------------------------

/// The version live at the horizon.
pub struct NeedBytes {
    version: Version,
}
impl DenyListFieldState for NeedBytes {}

impl DenyListFieldReader<NeedBytes> {
    /// The bytes at exactly that version. Pruning keeps every version a
    /// commit above the horizon consumed, so a miss is a storage error,
    /// never an answer.
    pub fn load_bytes(self, ctx: &CommitIndexedReader) -> IotaResult<DenyListField> {
        let key = ObjectKey(self.id, self.state.version);
        ctx.cache
            .try_get_object_by_key(&key.0, key.1)?
            .map(DenyListField::Live)
            .ok_or_else(|| IotaError::Storage(format!("no bytes for {key:?} in the object store")))
    }
}

// ---------------------------------------------------------------------------
// Pure comparison
// ---------------------------------------------------------------------------

/// What one row says about the field at the horizon.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum RowClass {
    /// Live at or below the horizon: its version is the one at the horizon.
    Live,
    /// Deleted or wrapped at or below the horizon.
    Absent,
    /// Produced above the horizon, so walked past.
    AboveHorizon,
}

fn classify_row(row: &HandlerProcessedObject, horizon: CommitIndex) -> RowClass {
    if row.produced_at > horizon {
        return RowClass::AboveHorizon;
    }
    match row.kind {
        HandlerProcessedObjectKind::Live => RowClass::Live,
        HandlerProcessedObjectKind::Deleted | HandlerProcessedObjectKind::Wrapped => {
            RowClass::Absent
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectDigest;

    use super::*;

    const HORIZON: CommitIndex = 10;

    fn row(kind: HandlerProcessedObjectKind, produced_at: CommitIndex) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind,
            produced_at,
            initial_shared_version: None,
            replaced_version: None,
            deleted_by: None,
        }
    }

    #[test]
    fn row_at_the_horizon_decides_and_one_above_is_walked_past() {
        assert_eq!(
            classify_row(&row(HandlerProcessedObjectKind::Live, HORIZON), HORIZON),
            RowClass::Live
        );
        assert_eq!(
            classify_row(&row(HandlerProcessedObjectKind::Live, HORIZON + 1), HORIZON),
            RowClass::AboveHorizon
        );
        assert_eq!(
            classify_row(
                &row(HandlerProcessedObjectKind::Deleted, HORIZON + 1),
                HORIZON
            ),
            RowClass::AboveHorizon
        );
    }

    #[test]
    fn tombstone_at_or_below_the_horizon_is_absent() {
        assert_eq!(
            classify_row(&row(HandlerProcessedObjectKind::Deleted, HORIZON), HORIZON),
            RowClass::Absent
        );
        assert_eq!(
            classify_row(
                &row(HandlerProcessedObjectKind::Wrapped, HORIZON - 1),
                HORIZON
            ),
            RowClass::Absent
        );
    }
}
