// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::StructTag;
use iota_types::{
    base_types::TransactionDigest,
    coin::Coin,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    move_package::MovePackage,
    object::{Data, MoveObject, MoveObjectExt, Object, ObjectInner, Owner},
    storage::ObjectKey,
};
use serde::{Deserialize, Serialize};

// Versioning process:
//
// Object storage versioning is done lazily (at read time) - therefore we must
// always preserve the code for reading the very first storage version. For all
// versions, a migration function
//
//   f(V_n) -> V_(n+1)
//
// must be defined. This way we can iteratively migrate the very oldest version
// to the very newest version at any point in the future.
//
// To change the format of the object table value types (StoreObject and
// StoreMoveObject), use the following process:
// - Add a new variant to the enum to store the new version type.
// - Define `From<StoreObjectV{N}> for StoreObjectV{N+1}` to update older
//   versions, and extend `migrate()` to chain `V{N}` -> `V{N+1}`.
// - Advance `pub type StoreObject = StoreObjectV{N+1}` and update
//   `From<StoreObject> for StoreObjectWrapper` to wrap the new variant.
// - Update `get_store_object` (and any other writers) to construct the new
//   value type directly.

/// Enum wrapper for versioning
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub enum StoreObjectWrapper {
    V1(StoreObjectV1),
    V2(StoreObjectV2),
}

// always points to latest version.
pub type StoreObject = StoreObjectV2;

impl StoreObjectWrapper {
    pub fn migrate(self) -> Self {
        match self {
            Self::V1(v1) => Self::V2(v1.into()),
            v2 @ Self::V2(_) => v2,
        }
    }

    // Always returns the most recent version. Older versions are migrated to the
    // latest version at read time, so there is never a need to access older
    // versions.
    pub fn inner(&self) -> &StoreObject {
        match self {
            Self::V1(_) => {
                panic!("object should have been migrated to latest version at read time")
            }
            Self::V2(v2) => v2,
        }
    }
    pub fn into_inner(self) -> StoreObject {
        match self {
            Self::V1(_) => {
                panic!("object should have been migrated to latest version at read time")
            }
            Self::V2(v2) => v2,
        }
    }
}

impl From<StoreObject> for StoreObjectWrapper {
    fn from(o: StoreObject) -> Self {
        StoreObjectWrapper::V2(o)
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub enum StoreObjectV1 {
    Value(Box<StoreObjectValue>),
    Deleted,
    Wrapped,
}

/// Forked version of [`iota_types::object::Object`]
/// Used for efficient storing of move objects in the database
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub struct StoreObjectValue {
    pub data: StoreData,
    pub owner: Owner,
    pub previous_transaction: TransactionDigest,
    pub storage_rebate: u64,
}

#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub enum StoreObjectV2 {
    Value(Box<StoreObjectValueV2>),
    Deleted,
    Wrapped,
}

/// V2 of [`StoreObjectValue`]. Adds `previous_transaction_checkpoint`,
/// the checkpoint sequence number that contained `previous_transaction`.
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub struct StoreObjectValueV2 {
    pub data: StoreData,
    pub owner: Owner,
    pub previous_transaction: TransactionDigest,
    /// Checkpoint sequence number of the checkpoint that contained
    /// `previous_transaction`. Only needed for the formal snapshot writer. The
    /// snapshot writer refuses to publish if any live-set row carries
    /// `None`; a node that wants to publish V2 snapshots must therefore
    /// have synced from genesis under V2 or been started from a V2 state so
    /// that no lifted-V1 rows are present.
    pub previous_transaction_checkpoint: Option<CheckpointSequenceNumber>,
    pub storage_rebate: u64,
}

impl From<StoreObjectV1> for StoreObjectV2 {
    fn from(v1: StoreObjectV1) -> Self {
        match v1 {
            StoreObjectV1::Value(v1_value) => Self::Value(Box::new(StoreObjectValueV2 {
                data: v1_value.data,
                owner: v1_value.owner,
                previous_transaction: v1_value.previous_transaction,
                // Pre-V2 rows never recorded the containing checkpoint;
                // there is no way to recover it. The snapshot writer
                // rejects any row carrying `None` rather than emit an
                // unknown value into the wire format.
                previous_transaction_checkpoint: None,
                storage_rebate: v1_value.storage_rebate,
            })),
            StoreObjectV1::Deleted => Self::Deleted,
            StoreObjectV1::Wrapped => Self::Wrapped,
        }
    }
}

/// Forked version of [`iota_types::object::Data`]
/// Adds extra enum value `IndirectObject`, which represents a reference to an
/// object stored separately
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub enum StoreData {
    Move(MoveObject),
    Package(MovePackage),
    IndirectObjectDeprecated,
    Coin(u64),
}

/// Build a `StoreObjectWrapper` for a newly written object version. The caller
/// supplies the checkpoint sequence number that contains the transaction whose
/// effects produced this object version.
pub fn get_store_object(
    object: Object,
    previous_transaction_checkpoint: Option<CheckpointSequenceNumber>,
) -> StoreObjectWrapper {
    let object = object.into_inner();

    let data = match object.data {
        Data::Package(package) => StoreData::Package(package),
        Data::Struct(move_obj) => {
            if move_obj.struct_tag().is_gas_coin() {
                StoreData::Coin(
                    Coin::from_bcs_bytes(move_obj.contents())
                        .expect("failed to deserialize coin")
                        .balance
                        .value(),
                )
            } else {
                StoreData::Move(move_obj)
            }
        }
    };
    let store_object = StoreObjectValueV2 {
        data,
        owner: object.owner,
        previous_transaction: object.previous_transaction,
        previous_transaction_checkpoint,
        storage_rebate: object.storage_rebate,
    };
    StoreObject::Value(Box::new(store_object)).into()
}

pub(crate) fn try_construct_object(
    object_key: &ObjectKey,
    store_object: StoreObjectValueV2,
) -> Result<Object, IotaError> {
    let data = match store_object.data {
        StoreData::Move(object) => Data::Struct(object),
        StoreData::Package(package) => Data::Package(package),
        StoreData::Coin(balance) => Data::Struct(MoveObject::new_from_execution_with_limit(
            StructTag::new_gas_coin(),
            object_key.1,
            bcs::to_bytes(&(object_key.0, balance)).expect("serialization failed"),
            u64::MAX,
        )?),
        _ => {
            return Err(IotaError::Storage(
                "corrupted field: inconsistent object representation".to_string(),
            ));
        }
    };

    Ok(ObjectInner {
        data,
        owner: store_object.owner,
        previous_transaction: store_object.previous_transaction,
        storage_rebate: store_object.storage_rebate,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use iota_types::base_types::TransactionDigest;

    use super::*;

    fn v1_value() -> StoreObjectValue {
        StoreObjectValue {
            data: StoreData::Coin(42),
            owner: Owner::Immutable,
            previous_transaction: TransactionDigest::random(),
            storage_rebate: 7,
        }
    }

    #[test]
    fn migrate_v1_value_lifts_with_none_checkpoint() {
        let v1 = v1_value();
        let wrapped = StoreObjectWrapper::V1(StoreObjectV1::Value(Box::new(v1.clone()))).migrate();
        let StoreObjectWrapper::V2(StoreObjectV2::Value(v2_value)) = wrapped else {
            panic!("expected V2(Value), got {wrapped:?}");
        };
        assert_eq!(v2_value.data, v1.data);
        assert_eq!(v2_value.owner, v1.owner);
        assert_eq!(v2_value.previous_transaction, v1.previous_transaction);
        assert_eq!(v2_value.storage_rebate, v1.storage_rebate);
        // Pre-V2 rows never recorded the checkpoint; lift surfaces that as
        // `None`. The snapshot writer rejects such rows at the boundary.
        assert_eq!(v2_value.previous_transaction_checkpoint, None);
    }

    #[test]
    fn migrate_v1_tombstones_lift_to_v2_tombstones() {
        let deleted = StoreObjectWrapper::V1(StoreObjectV1::Deleted).migrate();
        assert!(matches!(
            deleted,
            StoreObjectWrapper::V2(StoreObjectV2::Deleted)
        ));

        let wrapped = StoreObjectWrapper::V1(StoreObjectV1::Wrapped).migrate();
        assert!(matches!(
            wrapped,
            StoreObjectWrapper::V2(StoreObjectV2::Wrapped)
        ));
    }

    #[test]
    fn v1_wrapper_bcs_stays_at_discriminant_zero() {
        // Critical invariant: V1 must remain at BCS discriminant 0 so existing
        // on-disk V1 rows decode correctly after V2 was added to the enum.
        // Reordering StoreObjectWrapper variants would break this.
        let v1 = StoreObjectWrapper::V1(StoreObjectV1::Value(Box::new(v1_value())));
        let bytes = bcs::to_bytes(&v1).unwrap();
        assert_eq!(bytes[0], 0, "V1 must remain at BCS discriminant 0");

        let decoded: StoreObjectWrapper = bcs::from_bytes(&bytes).unwrap();
        assert!(matches!(decoded, StoreObjectWrapper::V1(_)));
    }

    /// `get_store_object` is the single production write site for new V2
    /// rows. `WritebackCache` always passes `Some(seq)` at checkpoint commit
    /// time. This locks that whatever the caller passes ends up faithfully
    /// on the row, for both `None` and a concrete `Some(seq)`.
    #[test]
    fn get_store_object_stamps_provided_checkpoint() {
        for expected in [None, Some(0xCAFE_F00D_BEEF_0001u64)] {
            let object =
                Object::immutable_with_id_for_testing(iota_types::base_types::ObjectID::random());
            let wrapper = get_store_object(object, expected);
            let StoreObjectWrapper::V2(StoreObjectV2::Value(value)) = wrapper else {
                panic!("expected V2(Value), got {wrapper:?}");
            };
            assert_eq!(
                value.previous_transaction_checkpoint, expected,
                "get_store_object must stamp the caller-provided checkpoint"
            );
        }
    }

    #[test]
    fn migrate_v2_is_identity() {
        let v2 = StoreObjectV2::Value(Box::new(StoreObjectValueV2 {
            data: StoreData::Coin(1),
            owner: Owner::Immutable,
            previous_transaction: TransactionDigest::random(),
            previous_transaction_checkpoint: Some(100),
            storage_rebate: 0,
        }));
        let wrapper = StoreObjectWrapper::V2(v2.clone());
        let migrated = wrapper.migrate();
        let StoreObjectWrapper::V2(out) = migrated else {
            panic!("V2 should remain V2 after migrate(), got {migrated:?}");
        };
        assert_eq!(out, v2);
    }

    /// Locks the BCS field-order layout of `StoreObjectValueV2` against a
    /// golden byte vector. Reordering or renaming any field would silently
    /// corrupt every on-disk V2 row; this test fails loudly on any such
    /// change.
    #[test]
    fn store_object_value_v2_bcs_layout_is_locked() {
        // `Some(seq)` case - the production write path.
        let v2 = StoreObjectValueV2 {
            data: StoreData::Coin(0x0102_0304_0506_0708),
            owner: Owner::Immutable,
            previous_transaction: TransactionDigest::ZERO,
            previous_transaction_checkpoint: Some(0x1011_1213_1415_1617),
            storage_rebate: 0x2021_2223_2425_2627,
        };
        let bytes = bcs::to_bytes(&v2).unwrap();

        let mut golden: Vec<u8> = Vec::new();
        // `StoreData::Coin(u64)` - the four-variant `StoreData` enum places
        // `Coin` at variant tag 3, followed by the u64 in little-endian.
        golden.push(0x03);
        golden.extend_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        // `Owner::Immutable` - Owner uses a custom serializer that maps to
        // `ReadableOwner` (AddressOwner=0, ObjectOwner=1, Shared=2,
        // Immutable=3), so Immutable encodes as a single tag byte.
        golden.push(0x03);
        // `TransactionDigest::ZERO` - Digest's binary BCS form is
        // length-prefixed: ULEB128 length 32 (=`0x20`) + 32 zero bytes.
        golden.push(0x20);
        golden.extend_from_slice(&[0u8; 32]);
        // `previous_transaction_checkpoint: Option<u64>` - `Some` discriminant
        // (0x01) + u64 in little-endian.
        golden.push(0x01);
        golden.extend_from_slice(&0x1011_1213_1415_1617u64.to_le_bytes());
        // `storage_rebate: u64` - little-endian.
        golden.extend_from_slice(&0x2021_2223_2425_2627u64.to_le_bytes());

        assert_eq!(
            bytes, golden,
            "StoreObjectValueV2 BCS layout changed; introduce a new StoreObject \
             version rather than mutating V2"
        );

        // `None` case - rows lifted from pre-V2 on-disk format.
        let v2_none = StoreObjectValueV2 {
            previous_transaction_checkpoint: None,
            ..v2
        };
        let bytes_none = bcs::to_bytes(&v2_none).unwrap();
        let mut golden_none: Vec<u8> = Vec::new();
        golden_none.push(0x03);
        golden_none.extend_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        golden_none.push(0x03);
        golden_none.push(0x20);
        golden_none.extend_from_slice(&[0u8; 32]);
        // `previous_transaction_checkpoint: Option<u64>` - `None` discriminant.
        golden_none.push(0x00);
        golden_none.extend_from_slice(&0x2021_2223_2425_2627u64.to_le_bytes());
        assert_eq!(bytes_none, golden_none);
    }
}
