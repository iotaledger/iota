// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Full-node-specific storage traits.
//!
//! This crate defines the [`GrpcStateReader`] and [`GrpcIndexes`] traits, which
//! extend the core [`ObjectStore`] and [`ReadStore`] interfaces from
//! `iota-types` with queries that are only available on full nodes (e.g. epoch
//! info, transaction-to-checkpoint mapping, type layout resolution).
//!
//! These traits live in a dedicated lightweight crate so that:
//! - `iota-core` can implement them without depending on `iota-grpc-server`
//! - `iota-grpc-server` can consume them without depending on `iota-core`
//! - `simulacrum` and other test harnesses can implement them freely

use iota_types::{
    base_types::{EpochId, IotaAddress, ObjectID},
    digests::{ChainIdentifier, TransactionDigest},
    messages_checkpoint::{CheckpointSequenceNumber, VerifiedCheckpoint},
    storage::{
        CoinInfoV2, DynamicFieldIteratorItem, EpochInfo, ObjectStore, OwnedObjectV2Cursor,
        OwnedObjectV2IteratorItem, PackageVersionIteratorItem, ReadStore, TransactionInfo,
        error::Result,
    },
};
use move_core_types::language_storage::{StructTag, TypeTag};

/// Trait extending [`ReadStore`] with full-node-specific queries that may
/// require richer databases or indexes to support.
pub trait GrpcStateReader: ObjectStore + ReadStore + Send + Sync {
    /// Lowest available checkpoint for which object data can be requested.
    ///
    /// Specifically this is the lowest checkpoint for which input/output object
    /// data will be available.
    fn get_lowest_available_checkpoint_objects(&self) -> Result<CheckpointSequenceNumber>;

    fn get_chain_identifier(&self) -> Result<ChainIdentifier>;

    fn get_epoch_last_checkpoint(&self, epoch_id: EpochId) -> Result<Option<VerifiedCheckpoint>>;

    /// Get a handle to an instance of the gRPC indexes.
    fn grpc_indexes(&self) -> Option<&dyn GrpcIndexes>;

    fn get_type_layout(
        &self,
        type_tag: &TypeTag,
    ) -> Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        use move_core_types::annotated_value::MoveTypeLayout;
        match type_tag {
            TypeTag::Bool => Ok(Some(MoveTypeLayout::Bool)),
            TypeTag::U8 => Ok(Some(MoveTypeLayout::U8)),
            TypeTag::U64 => Ok(Some(MoveTypeLayout::U64)),
            TypeTag::U128 => Ok(Some(MoveTypeLayout::U128)),
            TypeTag::Address => Ok(Some(MoveTypeLayout::Address)),
            TypeTag::Signer => Ok(Some(MoveTypeLayout::Signer)),
            TypeTag::Vector(type_tag) => Ok(self
                .get_type_layout(type_tag)?
                .map(|layout| MoveTypeLayout::Vector(Box::new(layout)))),
            TypeTag::Struct(struct_tag) => self.get_struct_layout(struct_tag),
            TypeTag::U16 => Ok(Some(MoveTypeLayout::U16)),
            TypeTag::U32 => Ok(Some(MoveTypeLayout::U32)),
            TypeTag::U256 => Ok(Some(MoveTypeLayout::U256)),
        }
    }

    fn get_struct_layout(
        &self,
        type_tag: &StructTag,
    ) -> Result<Option<move_core_types::annotated_value::MoveTypeLayout>>;
}

/// GRPC-index-specific queries.
///
/// Provides access to the on-disk indexes needed by the gRPC server:
/// epoch info, transaction-to-checkpoint mapping, owned objects, dynamic
/// fields, coin info, and package versions.
pub trait GrpcIndexes: Send + Sync {
    fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfo>>;

    fn get_transaction_info(&self, digest: &TransactionDigest) -> Result<Option<TransactionInfo>>;

    /// Iterate over objects owned by `owner` using the v2 index, optionally
    /// filtered by `object_type`.
    ///
    /// Each item includes an [`OwnedObjectV2Cursor`] for seek-based pagination.
    /// The `cursor` bound is **inclusive**.
    fn account_owned_objects_info_iter_v2(
        &self,
        owner: IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = OwnedObjectV2IteratorItem> + '_>>;

    /// Iterate over the dynamic fields of `parent`.
    ///
    /// The `cursor` bound is **inclusive**.
    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> Result<Box<dyn Iterator<Item = DynamicFieldIteratorItem> + '_>>;

    /// Get unified coin info from the `coin_v2` table.
    fn get_coin_v2_info(&self, coin_type: &StructTag) -> Result<Option<CoinInfoV2>>;

    /// Iterate over all versions of a package by its original package ID.
    fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>>;

    /// Returns `true` once the `owner_v2` backfill has completed.
    fn is_owner_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `coin_v2` backfill has completed.
    fn is_coin_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `package_version` backfill has completed.
    fn is_package_version_index_ready(&self) -> bool {
        true
    }
}
