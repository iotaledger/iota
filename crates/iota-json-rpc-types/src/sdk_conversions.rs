// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions from `iota_json_rpc_types` into `iota-rust-sdk`-native types.
//!
//! These live on the JSON-RPC side (not the gRPC side) because the gRPC
//! client already produces SDK-native types directly; this module only
//! exists to give `WalletContext`'s JSON-RPC fallback path the same return
//! types as its gRPC path.

use iota_grpc_types::v1::{
    transaction::{
        BalanceChange as ProtoBalanceChange, BalanceChanges as ProtoBalanceChanges,
        ExecutedTransaction, ObjectChange as ProtoObjectChange,
        ObjectChangeCreated as ProtoObjectChangeCreated,
        ObjectChangeDeleted as ProtoObjectChangeDeleted,
        ObjectChangeMutated as ProtoObjectChangeMutated,
        ObjectChangePublished as ProtoObjectChangePublished,
        ObjectChangeUnwrapped as ProtoObjectChangeUnwrapped,
        ObjectChangeWrapped as ProtoObjectChangeWrapped, ObjectChanges as ProtoObjectChanges,
        Transaction as ProtoTransaction, TransactionEffects as ProtoTransactionEffects,
        object_change::Kind as ProtoObjectChangeKind,
    },
    types::Digest as ProtoDigest,
};
use iota_sdk_types::{MoveObjectType, MoveStruct, Object, ObjectData, StructTag, Version};

use crate::{BalanceChange, IotaObjectData, IotaTransactionBlockResponse, ObjectChange};

/// Error converting between `iota_json_rpc_types` and `iota-rust-sdk`-native
/// types.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SdkConversionError(pub String);

impl From<bcs::Error> for SdkConversionError {
    fn from(value: bcs::Error) -> Self {
        Self(value.to_string())
    }
}

impl TryFrom<&IotaObjectData> for Object {
    type Error = SdkConversionError;

    fn try_from(value: &IotaObjectData) -> Result<Self, Self::Error> {
        let owner = value
            .owner
            .ok_or_else(|| SdkConversionError("missing owner (request with_owner())".into()))?;
        let previous_transaction = value.previous_transaction.ok_or_else(|| {
            SdkConversionError(
                "missing previous_transaction (request with_previous_transaction())".into(),
            )
        })?;
        let storage_rebate = value.storage_rebate.ok_or_else(|| {
            SdkConversionError("missing storage_rebate (request with_storage_rebate())".into())
        })?;
        let raw = value
            .bcs
            .as_ref()
            .ok_or_else(|| SdkConversionError("missing bcs (request with_bcs())".into()))?;

        let data = match raw {
            crate::iota_object::IotaRawData::MoveObject(raw_move_object) => ObjectData::Struct(
                MoveStruct::new(
                    MoveObjectType::new(raw_move_object.type_.clone()),
                    raw_move_object.version.into(),
                    raw_move_object.bcs_bytes.clone(),
                )
                .map_err(|e| SdkConversionError(e.to_string()))?,
            ),
            crate::iota_object::IotaRawData::Package(_) => {
                return Err(SdkConversionError(
                    "converting a package IotaObjectData to iota_sdk_types::Object is not \
                     supported"
                        .into(),
                ));
            }
        };

        Ok(Object {
            data,
            owner,
            previous_transaction,
            storage_rebate,
        })
    }
}

impl TryFrom<&BalanceChange> for ProtoBalanceChange {
    type Error = SdkConversionError;

    fn try_from(value: &BalanceChange) -> Result<Self, Self::Error> {
        Ok(ProtoBalanceChange::default()
            .with_owner(value.owner)
            .with_coin_type(&value.coin_type)
            .with_amount(prost::bytes::Bytes::copy_from_slice(
                &value.amount.to_be_bytes(),
            )))
    }
}

/// Converts JSON-RPC's already-computed `ObjectChange` onto the equivalent
/// gRPC proto `ObjectChange` variant.
///
/// `ObjectChange::Transferred` has no gRPC proto equivalent (the proto schema
/// carries only
/// `Published`/`Mutated`/`Deleted`/`Wrapped`/`Unwrapped`/`Created`),
/// so it converts to an error. No consumer in this repo reads a `Transferred`
/// change through a `WalletContext`-returned response, so this is an
/// unexercised limitation of the JSON-RPC fallback path.
impl TryFrom<&ObjectChange> for ProtoObjectChange {
    type Error = SdkConversionError;

    fn try_from(value: &ObjectChange) -> Result<Self, Self::Error> {
        Ok(match value {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => ProtoObjectChange::default().with_published(
                ProtoObjectChangePublished::default()
                    .with_package_id(*package_id)
                    .with_version((*version).as_u64())
                    .with_digest(*digest)
                    .with_modules(modules.clone()),
            ),
            ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => ProtoObjectChange::default().with_mutated(
                ProtoObjectChangeMutated::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).as_u64())
                    .with_previous_version((*previous_version).as_u64())
                    .with_digest(*digest),
            ),
            ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => ProtoObjectChange::default().with_deleted(
                ProtoObjectChangeDeleted::default()
                    .with_sender(*sender)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).as_u64()),
            ),
            ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => ProtoObjectChange::default().with_wrapped(
                ProtoObjectChangeWrapped::default()
                    .with_sender(*sender)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).as_u64()),
            ),
            ObjectChange::Unwrapped {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => ProtoObjectChange::default().with_unwrapped(
                ProtoObjectChangeUnwrapped::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).as_u64())
                    .with_digest(*digest),
            ),
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => ProtoObjectChange::default().with_created(
                ProtoObjectChangeCreated::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).as_u64())
                    .with_digest(*digest),
            ),
            ObjectChange::Transferred { .. } => {
                return Err(SdkConversionError(
                    "ObjectChange::Transferred has no gRPC proto equivalent".into(),
                ));
            }
        })
    }
}

fn decode_transaction_bcs(
    raw_transaction: &[u8],
) -> Result<iota_sdk_types::Transaction, SdkConversionError> {
    let sender_signed_data: iota_types::transaction::SenderSignedData =
        bcs::from_bytes(raw_transaction)
            .map_err(|e| SdkConversionError(format!("decoding raw_transaction: {e}")))?;
    let signed: iota_sdk_types::SignedTransaction = sender_signed_data.try_into().map_err(
        |e: iota_types::iota_sdk_types_conversions::SdkTypeConversionError| {
            SdkConversionError(e.to_string())
        },
    )?;
    Ok(signed.transaction)
}

/// Converts a JSON-RPC transaction response into the gRPC-native
/// `ExecutedTransaction` so the JSON-RPC fallback path returns the same type
/// the gRPC path produces.
///
/// Only `transaction.digest`/`transaction.bcs`/`effects.bcs`/`checkpoint`/
/// `timestamp`/`object_changes`/`balance_changes` are populated;
/// `effects.digest` and `events` are not, since no consumer in this repo reads
/// them off a `WalletContext`-returned response. Returns an error if the
/// response carries an `ObjectChange::Transferred` (no gRPC equivalent).
impl TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction {
    type Error = SdkConversionError;

    fn try_from(value: &IotaTransactionBlockResponse) -> Result<Self, Self::Error> {
        let mut executed = ExecutedTransaction::default().with_transaction(
            ProtoTransaction::default()
                .with_digest(ProtoDigest::from(iota_sdk_types::Digest::from(
                    value.digest,
                )))
                .with_bcs(iota_grpc_types::v1::bcs::BcsData::serialize(
                    &decode_transaction_bcs(&value.raw_transaction)?,
                )?),
        );

        if !value.raw_effects.is_empty() {
            let effects: iota_sdk_types::TransactionEffects = bcs::from_bytes(&value.raw_effects)?;
            executed = executed.with_effects(
                ProtoTransactionEffects::default()
                    .with_bcs(iota_grpc_types::v1::bcs::BcsData::serialize(&effects)?),
            );
        }

        if let Some(checkpoint) = value.checkpoint {
            executed = executed.with_checkpoint(checkpoint);
        }
        if let Some(timestamp_ms) = value.timestamp_ms {
            executed = executed.with_timestamp(prost_types::Timestamp {
                seconds: (timestamp_ms / 1000) as i64,
                nanos: ((timestamp_ms % 1000) * 1_000_000) as i32,
            });
        }

        if let Some(object_changes) = &value.object_changes {
            executed = executed.with_object_changes(
                ProtoObjectChanges::default().with_object_changes(
                    object_changes
                        .iter()
                        .map(ProtoObjectChange::try_from)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            );
        }
        if let Some(balance_changes) = &value.balance_changes {
            executed = executed.with_balance_changes(
                ProtoBalanceChanges::default().with_balance_changes(
                    balance_changes
                        .iter()
                        .map(ProtoBalanceChange::try_from)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            );
        }

        Ok(executed)
    }
}

/// Converts a gRPC-native `ExecutedTransaction` back into a JSON-RPC
/// `IotaTransactionBlockResponse` for consumers that still expect the JSON-RPC
/// shape (the CLI's shared display path and
/// `TestCluster::execute_transaction`).
///
/// Populates the digest, `effects` (and `raw_effects`), `object_changes`,
/// `balance_changes`, `checkpoint`, and `timestamp_ms` when present on the
/// source; `transaction`/`raw_transaction`/`events` are left at their zero
/// values, which no consumer of this direction reads.
impl TryFrom<&ExecutedTransaction> for IotaTransactionBlockResponse {
    type Error = SdkConversionError;

    fn try_from(value: &ExecutedTransaction) -> Result<Self, Self::Error> {
        let digest = value
            .transaction()
            .map_err(|e| SdkConversionError(e.to_string()))?
            .digest()
            .map_err(|e| SdkConversionError(e.to_string()))?;

        let (effects, raw_effects) = match value.effects().ok().and_then(|e| e.effects().ok()) {
            Some(sdk_effects) => {
                let raw = bcs::to_bytes(&sdk_effects)?;
                let effects = crate::IotaTransactionBlockEffects::try_from(sdk_effects)
                    .map_err(|e| SdkConversionError(e.to_string()))?;
                (Some(effects), raw)
            }
            None => (None, vec![]),
        };

        let object_changes = value
            .object_changes()
            .ok()
            .map(|changes| {
                changes
                    .object_changes
                    .iter()
                    .map(proto_object_change_to_json_rpc)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let balance_changes = value
            .balance_changes()
            .ok()
            .map(|changes| {
                changes
                    .balance_changes
                    .iter()
                    .map(proto_balance_change_to_json_rpc)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(IotaTransactionBlockResponse {
            digest,
            transaction: None,
            raw_transaction: vec![],
            effects,
            events: None,
            object_changes,
            balance_changes,
            timestamp_ms: value.timestamp_ms().ok(),
            confirmed_local_execution: None,
            checkpoint: value.checkpoint,
            errors: vec![],
            raw_effects,
        })
    }
}

fn proto_balance_change_to_json_rpc(
    value: &ProtoBalanceChange,
) -> Result<BalanceChange, SdkConversionError> {
    Ok(BalanceChange {
        owner: value
            .owner()
            .map_err(|e| SdkConversionError(e.to_string()))?,
        coin_type: value
            .coin_type()
            .map_err(|e| SdkConversionError(e.to_string()))?,
        amount: value
            .amount_i128()
            .map_err(|e| SdkConversionError(e.to_string()))?,
    })
}

fn struct_tag_from_type_tag(
    type_tag: iota_sdk_types::TypeTag,
    context: &str,
) -> Result<StructTag, SdkConversionError> {
    type_tag
        .as_struct_tag_opt()
        .cloned()
        .ok_or_else(|| SdkConversionError(format!("{context} object_type is not a struct")))
}

fn missing_version() -> SdkConversionError {
    SdkConversionError("missing version".into())
}

fn proto_object_change_to_json_rpc(
    value: &ProtoObjectChange,
) -> Result<ObjectChange, SdkConversionError> {
    let err = |e: iota_grpc_types::proto::TryFromProtoError| SdkConversionError(e.to_string());
    match value
        .kind
        .as_ref()
        .ok_or_else(|| SdkConversionError("ObjectChange has no populated kind".into()))?
    {
        ProtoObjectChangeKind::Published(c) => Ok(ObjectChange::Published {
            package_id: c.package_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest().map_err(err)?,
            modules: c.modules.clone(),
        }),
        ProtoObjectChangeKind::Mutated(c) => Ok(ObjectChange::Mutated {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: struct_tag_from_type_tag(c.object_type().map_err(err)?, "mutated")?,
            object_id: c.object_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            previous_version: Version::from_u64(
                c.previous_version
                    .ok_or_else(|| SdkConversionError("missing previous_version".into()))?,
            ),
            digest: c.digest().map_err(err)?,
        }),
        ProtoObjectChangeKind::Deleted(c) => Ok(ObjectChange::Deleted {
            sender: c.sender().map_err(err)?,
            object_type: struct_tag_from_type_tag(c.object_type().map_err(err)?, "deleted")?,
            object_id: c.object_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
        }),
        ProtoObjectChangeKind::Wrapped(c) => Ok(ObjectChange::Wrapped {
            sender: c.sender().map_err(err)?,
            object_type: struct_tag_from_type_tag(c.object_type().map_err(err)?, "wrapped")?,
            object_id: c.object_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
        }),
        ProtoObjectChangeKind::Unwrapped(c) => Ok(ObjectChange::Unwrapped {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: struct_tag_from_type_tag(c.object_type().map_err(err)?, "unwrapped")?,
            object_id: c.object_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest().map_err(err)?,
        }),
        ProtoObjectChangeKind::Created(c) => Ok(ObjectChange::Created {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: struct_tag_from_type_tag(c.object_type().map_err(err)?, "created")?,
            object_id: c.object_id().map_err(err)?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest().map_err(err)?,
        }),
        _ => Err(SdkConversionError("unknown ObjectChange kind".into())),
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, GenesisTransaction, Owner, TransactionDigest, TransactionKind};
    use iota_types::transaction::{SenderSignedData, TransactionData, TransactionDataAPI};

    use super::*;
    use crate::iota_object::{IotaRawData, IotaRawMoveObject};

    fn address(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    /// A gas- and signature-free genesis system transaction, enough to
    /// exercise the digest/BCS plumbing of the conversions.
    fn sample_transaction() -> TransactionData {
        TransactionData::new_system_transaction(TransactionKind::Genesis(GenesisTransaction {
            objects: vec![],
            events: vec![],
        }))
    }

    fn sample_iota_object_data() -> IotaObjectData {
        let object_id = iota_sdk_types::ObjectId::random();
        let mut contents = object_id.as_bytes().to_vec();
        contents.extend_from_slice(&[0u8; 8]); // opaque Move-struct payload
        IotaObjectData {
            object_id,
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
            type_: None,
            owner: Some(Owner::Address(address(2))),
            previous_transaction: Some(TransactionDigest::random()),
            storage_rebate: Some(0),
            display: None,
            content: None,
            bcs: Some(IotaRawData::MoveObject(IotaRawMoveObject {
                type_: StructTag::new_gas_coin(),
                version: Version::from_u64(1).into(),
                bcs_bytes: contents,
            })),
        }
    }

    #[test]
    fn object_conversion_round_trips_owner_and_previous_transaction() {
        let data = sample_iota_object_data();
        let object = Object::try_from(&data).unwrap();
        assert_eq!(object.owner(), &data.owner.unwrap());
        assert_eq!(
            object.previous_transaction(),
            data.previous_transaction.unwrap()
        );
        assert_eq!(object.storage_rebate(), data.storage_rebate.unwrap());
    }

    #[test]
    fn object_conversion_requires_bcs() {
        let mut data = sample_iota_object_data();
        data.bcs = None;
        assert!(Object::try_from(&data).is_err());
    }

    #[test]
    fn object_change_created_maps_to_proto_created() {
        let sender = Address::from(iota_sdk_types::ObjectId::random());
        let owner = Owner::Address(sender);
        let object_id = iota_sdk_types::ObjectId::random();
        let digest = iota_sdk_types::ObjectDigest::random();
        let change = ObjectChange::Created {
            sender,
            owner,
            object_type: StructTag::new_gas_coin(),
            object_id,
            version: 1.into(),
            digest,
        };

        let proto = ProtoObjectChange::try_from(&change).unwrap();
        let ProtoObjectChangeKind::Created(created) = proto.kind.as_ref().unwrap() else {
            panic!("expected a Created object change");
        };
        assert_eq!(created.sender().unwrap(), sender);
        assert_eq!(created.owner().unwrap(), owner);
        assert_eq!(created.object_id().unwrap(), object_id);
        assert_eq!(created.digest().unwrap(), digest);
    }

    #[test]
    fn object_change_transferred_is_unrepresentable() {
        let sender = Address::from(iota_sdk_types::ObjectId::random());
        let change = ObjectChange::Transferred {
            sender,
            recipient: Owner::Address(sender),
            object_type: StructTag::new_gas_coin(),
            object_id: iota_sdk_types::ObjectId::random(),
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
        };

        assert!(ProtoObjectChange::try_from(&change).is_err());
    }

    #[test]
    fn balance_change_round_trips_negative_amount() {
        let owner = Owner::Address(Address::from(iota_sdk_types::ObjectId::random()));
        let change = BalanceChange {
            owner,
            coin_type: iota_sdk_types::TypeTag::Struct(Box::new(StructTag::new_gas())),
            amount: -42,
        };

        let proto = ProtoBalanceChange::try_from(&change).unwrap();
        assert_eq!(proto.owner().unwrap(), owner);
        assert_eq!(proto.amount_i128().unwrap(), -42);
    }

    #[test]
    fn executed_transaction_conversion_round_trips_digest_and_object_changes() {
        let tx = sample_transaction();
        let digest = tx.digest();
        let package_id = iota_sdk_types::ObjectId::random();
        let change = ObjectChange::Published {
            package_id,
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
            modules: vec!["m".to_string()],
        };
        let response = IotaTransactionBlockResponse {
            digest,
            transaction: None,
            raw_transaction: bcs::to_bytes(&SenderSignedData::new(tx, vec![])).unwrap(),
            effects: None,
            events: None,
            object_changes: Some(vec![change]),
            balance_changes: Some(vec![]),
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: None,
            errors: vec![],
            raw_effects: vec![],
        };

        let executed = ExecutedTransaction::try_from(&response).unwrap();
        assert_eq!(executed.transaction().unwrap().digest().unwrap(), digest);
        let changes = executed.object_changes().unwrap();
        assert_eq!(changes.object_changes.len(), 1);
        let ProtoObjectChangeKind::Published(published) =
            changes.object_changes[0].kind.as_ref().unwrap()
        else {
            panic!("expected a Published object change");
        };
        assert_eq!(published.package_id().unwrap(), package_id);
    }

    #[test]
    fn reverse_conversion_round_trips_digest() {
        let digest = TransactionDigest::random();
        let executed = ExecutedTransaction::default().with_transaction(
            ProtoTransaction::default()
                .with_digest(ProtoDigest::from(iota_sdk_types::Digest::from(digest))),
        );

        let response = IotaTransactionBlockResponse::try_from(&executed).unwrap();
        assert_eq!(response.digest, digest);
    }
}
