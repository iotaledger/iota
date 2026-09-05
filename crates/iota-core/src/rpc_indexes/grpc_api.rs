// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The gRPC read surface of the unified RPC index store. Every public read
//! here fails with `StorageError::custom("the gRPC index group is not
//! enabled")` when the store does not maintain the [`IndexGroup::Grpc`]
//! group's tables.

use std::ops::Bound;

use iota_sdk_types::{Address, ObjectId, StructTag, TransactionDigest};
use iota_types::storage::{
    AccountOwnedObjectInfo, DynamicFieldIteratorItem, DynamicFieldKey, OwnedObjectCursor,
    OwnedObjectIteratorItem, PackageVersionIteratorItem, TransactionInfo,
    error::Error as StorageError,
};
use typed_store::{TypedStoreError, traits::Map};

use super::{
    RpcIndexesStore,
    schema::{CoinIndexInfo, CoinIndexKey, IndexGroup, OwnerIndexKey, OwnerTypeFilter},
};

impl From<CoinIndexInfo> for iota_types::storage::CoinInfo {
    fn from(info: CoinIndexInfo) -> Self {
        Self {
            coin_metadata_object_id: info.coin_metadata_object_id,
            treasury_object_id: info.treasury_object_id,
            regulated_coin_metadata_object_id: info.regulated_coin_metadata_object_id,
        }
    }
}

impl RpcIndexesStore {
    /// Fails fast when this store does not maintain the gRPC group's
    /// tables.
    fn require_grpc(&self) -> Result<(), StorageError> {
        if self.serves(IndexGroup::Grpc) {
            Ok(())
        } else {
            Err(StorageError::custom("the gRPC index group is not enabled"))
        }
    }

    /// The checkpoint containing `digest`, from the digest history buckets.
    ///
    /// An exact-key probe over the buckets, newest first; a miss in a sealed
    /// bucket is answered by its in-memory bloom filters. Digests of
    /// checkpoints pruned mid-epoch stay answerable until the whole epoch's
    /// bucket drops.
    pub fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, StorageError> {
        self.require_grpc()?;
        Ok(self
            .lookup_digest(digest)
            .map_err(|e| StorageError::custom(e.to_string()))?
            .map(|(_, checkpoint)| TransactionInfo {
                checkpoint,
                object_types: Default::default(),
            }))
    }

    /// The dynamic fields of `parent`, keyed on the field's own object id.
    /// Only the key is stored; field metadata is resolved on demand from the
    /// object store at query time.
    pub fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> Result<impl Iterator<Item = Result<DynamicFieldKey, TypedStoreError>> + '_, StorageError>
    {
        self.require_grpc()?;
        Ok(self
            .tables
            .dynamic_field
            .safe_iter_with_prefix_from(&parent, Bound::Included(&cursor.unwrap_or(ObjectId::ZERO)))
            .map(|r| r.map(|(key, ())| key)))
    }

    /// Regulated coin metadata for `coin_type`, `None` if the type has none.
    pub(crate) fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfo>, StorageError> {
        self.require_grpc()?;
        let key = CoinIndexKey {
            coin_type: coin_type.to_owned(),
        };
        Ok(self.tables.coin.get(&key)?)
    }

    /// Every storage id of `original_package_id`'s versions, from `cursor`
    /// (inclusive) on.
    pub fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> Result<impl Iterator<Item = PackageVersionIteratorItem> + '_, StorageError> {
        self.require_grpc()?;
        Ok(self.tables.package_version.safe_iter_with_prefix_from(
            &original_package_id,
            Bound::Included(&cursor.unwrap_or(0)),
        ))
    }
}

// ---------------------------------------------------------------------------
// GrpcIndexes trait implementation
// ---------------------------------------------------------------------------

impl iota_node_storage::GrpcIndexes for RpcIndexesStore {
    fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionInfo>> {
        RpcIndexesStore::get_transaction_info(self, digest)
    }

    fn account_owned_objects_info_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnedObjectCursor>,
        object_type: Option<StructTag>,
    ) -> iota_types::storage::error::Result<Box<dyn Iterator<Item = OwnedObjectIteratorItem> + '_>>
    {
        self.require_grpc()?;
        // `OwnedObjectCursor` carries every field of `OwnerIndexKey` but the
        // owner, which the caller already supplies separately.
        let cursor_key = cursor.map(|c| OwnerIndexKey {
            owner,
            object_type_identifier: c.object_type_identifier,
            object_type_params: c.object_type_params,
            inverted_balance: c.inverted_balance,
            object_id: c.object_id,
        });
        let type_filter = OwnerTypeFilter::from_struct_tag(object_type.as_ref());
        let iter = self
            .owner_iter(owner, cursor_key.as_ref(), type_filter)
            .map_err(|e| StorageError::custom(e.to_string()))?
            .map(|result| {
                result.map(|(key, info)| {
                    let cursor = OwnedObjectCursor {
                        object_type_identifier: key.object_type_identifier,
                        object_type_params: key.object_type_params,
                        inverted_balance: key.inverted_balance,
                        object_id: key.object_id,
                    };
                    let obj_info = AccountOwnedObjectInfo {
                        owner: key.owner,
                        object_id: key.object_id,
                        version: info.version,
                        object_type: info.object_type.into(),
                    };
                    (obj_info, cursor)
                })
            });
        Ok(Box::new(iter))
    }

    fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> iota_types::storage::error::Result<Box<dyn Iterator<Item = DynamicFieldIteratorItem> + '_>>
    {
        let iter = RpcIndexesStore::dynamic_field_iter(self, parent, cursor)?;
        Ok(Box::new(iter))
    }

    fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> iota_types::storage::error::Result<Option<iota_types::storage::CoinInfo>> {
        Ok(RpcIndexesStore::get_coin_info(self, coin_type)?.map(Into::into))
    }

    fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> iota_types::storage::error::Result<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>>
    {
        let iter = RpcIndexesStore::package_versions_iter(self, original_package_id, cursor)?;
        Ok(Box::new(iter))
    }
}
