// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and logic to interact with the db.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use cached::{Cached, SizedCache};
use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    OptionalExtension, PgConnection, QueryDsl, QueryableByName, RunQueryDsl, SelectableHelper,
    TextExpressionMethods,
    dsl::sql,
    r2d2::ConnectionManager,
    sql_query,
    sql_types::{BigInt, Bool},
};
use fastcrypto::encoding::{Encoding, Hex};
use futures::FutureExt;
use iota_json_rpc_types::{
    AddressMetrics, Balance, CheckpointId, Coin as IotaCoin, DisplayFieldsResponse, EpochInfo,
    EventFilter, IotaCoinMetadata, IotaEvent, IotaMoveValue, IotaObjectDataFilter,
    IotaObjectDataOptions, IotaObjectResponse, IotaTransactionBlockResponse, IotaTransactionKind,
    MoveCallMetrics, MoveFunctionName, NetworkMetrics, ParticipationMetrics, TransactionFilter,
    TransactionFilterV2,
};
use iota_package_resolver::{Package, PackageStore, PackageStoreWithLruCache, Resolver};
use iota_transaction_builder::DataReader;
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID, SequenceNumber, StructTag, TypeTag, VersionNumber},
    coin::TreasuryCap,
    coin_manager::CoinManager,
    committee::EpochId,
    digests::{ChainIdentifier, TransactionDigest},
    dynamic_field::{DynamicFieldInfo, DynamicFieldName, visitor as DFV},
    effects::TransactionEvents,
    error::IotaError,
    event::EventID,
    iota_sdk_types_conversions::type_tag_core_to_sdk,
    iota_system_state::{
        IotaSystemStateTrait,
        iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
    },
    messages_checkpoint::{CheckpointDigest, CheckpointSequenceNumber},
    object::{Object, ObjectRead, PastObjectRead, bounded_visitor::BoundedVisitor},
};
use itertools::Itertools;
use move_core_types::annotated_value::MoveStructLayout;
use tap::TapFallible;

use crate::{
    apis::GovernanceReadApi,
    db::{ConnectionConfig, ConnectionPool, ConnectionPoolConfig},
    errors::{Context, IndexerError},
    historical_fallback::reader::HistoricalFallbackReader,
    ingestion::common::persist::CommitterTables,
    models::{
        address_metrics::StoredAddressMetrics,
        checkpoints::{StoredChainIdentifier, StoredCheckpoint},
        display::StoredDisplay,
        epoch::StoredEpochInfo,
        events::StoredEvent,
        move_call_metrics::QueriedMoveCallMetrics,
        network_metrics::StoredNetworkMetrics,
        obj_indices::StoredObjectVersion,
        objects::{CoinBalance, StoredHistoryObject, StoredObject},
        participation_metrics::StoredParticipationMetrics,
        system_state::StoredSystemState,
        transactions::{
            OptimisticTransaction, StoredTransaction, StoredTransactionEvents,
            stored_events_to_events, tx_events_to_iota_tx_events,
        },
        tx_indices::TxSequenceNumber,
    },
    pruning::watermark_task::WatermarkCache,
    schema::{
        address_metrics, addresses, chain_identifier, checkpoints, display, epochs, events,
        objects, objects_history, objects_snapshot, objects_version, optimistic_transactions,
        packages, pruner_cp_watermark, transactions, tx_digests, tx_global_order,
    },
    store::{
        diesel_macro::{mark_in_blocking_pool, *},
        package_resolver::IndexerStorePackageResolver,
    },
    types::{IndexerResult, OwnerType},
};

pub const TX_SEQUENCE_NUMBER_STR: &str = "tx_sequence_number";
pub const GLOBAL_SEQUENCE_NUMBER_STR: &str = "global_sequence_number";
pub const OPTIMISTIC_SEQUENCE_NUMBER_STR: &str = "optimistic_sequence_number";
pub const TX_DIGEST_STR: &str = "tx_digest";
pub const EVENT_SEQUENCE_NUMBER_STR: &str = "event_sequence_number";

/// Result of checking input object dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputObjectsStatus {
    /// All objects exist at the requested versions and are finalized.
    Ready,
    /// At least one object has a newer version — waiting will never succeed.
    Superseded,
    /// Not all objects are ready yet (missing, older version, or not yet
    /// finalized by checkpoint indexing).
    Pending,
}

/// Encapsulates the logic for reading from the database.
///
/// Provides a set of methods to perform read operations,
/// including resolution of packages.
#[derive(Clone)]
pub struct IndexerReader {
    pool: ConnectionPool,
    package_resolver: PackageResolver,
    obj_type_cache: Arc<Mutex<SizedCache<String, Option<ObjectID>>>>,
    fallback: Option<HistoricalFallbackReader>,
    watermark_cache: WatermarkCache,
}

/// Encapsulates the logic for reading data from the database.
///
/// This reader only reads data from the DB (checkpointed or optimistic data)
/// and does not read historical fallback data from the key-value store.
pub struct DBReader<'a> {
    main_reader: &'a IndexerReader,
}

pub type PackageResolver = Arc<Resolver<PackageStoreWithLruCache<IndexerStorePackageResolver>>>;

// Impl for common initialization and utilities
impl IndexerReader {
    pub fn new(pool: ConnectionPool, watermark_cache: WatermarkCache) -> Self {
        let indexer_store_pkg_resolver = IndexerStorePackageResolver::new(pool.clone());
        let package_cache = PackageStoreWithLruCache::new(indexer_store_pkg_resolver);
        let package_resolver = Arc::new(Resolver::new(package_cache));
        let obj_type_cache = Arc::new(Mutex::new(SizedCache::with_size(10000)));
        Self {
            pool,
            package_resolver,
            obj_type_cache,
            fallback: None,
            watermark_cache,
        }
    }

    /// Creates a new IndexerReader without a watermark cache (for tests or
    /// non-pruning scenarios)
    pub fn new_without_watermark_cache(pool: ConnectionPool) -> Self {
        Self::new(pool, WatermarkCache::default())
    }

    /// Returns a [`DBReader`] bound to this `IndexerReader` instance which
    /// allows to perform database reads.
    pub fn db(&self) -> DBReader<'_> {
        DBReader::new(self)
    }

    pub fn new_with_config<T: Into<String>>(
        db_url: T,
        config: ConnectionPoolConfig,
        watermark_cache: WatermarkCache,
    ) -> Result<Self> {
        let manager = ConnectionManager::<PgConnection>::new(db_url);

        let connection_config = ConnectionConfig {
            statement_timeout: config.statement_timeout,
            read_only: true,
        };

        let pool = diesel::r2d2::Pool::builder()
            .max_size(config.pool_size)
            .connection_timeout(config.connection_timeout)
            .connection_customizer(Box::new(connection_config))
            .build(manager)
            .map_err(|e| anyhow!("failed to initialize connection pool. Error: {:?}. If Error is None, please check whether the configured pool size (currently {}) exceeds the maximum number of connections allowed by the database.", e, config.pool_size))?;

        Ok(Self::new(pool, watermark_cache))
    }

    /// Add a historical fallback reader to the indexer.
    ///
    /// In case the IndexerReader fails to retrieve data, the fallback reader
    /// will be used to retrieve the data.
    pub(crate) fn with_fallback_reader(&mut self, fallback: HistoricalFallbackReader) {
        self.fallback = Some(fallback);
    }

    /// Access the internal fallback reader.
    pub(crate) fn fallback_reader(&self) -> Option<&HistoricalFallbackReader> {
        self.fallback.as_ref()
    }

    /// Accesses the watermark cache.
    pub fn watermark_cache(&self) -> &WatermarkCache {
        &self.watermark_cache
    }

    /// Ensures that the specified tables have data available for the given
    /// checkpoint. Returns an error if any of the tables have been pruned
    /// for this checkpoint.
    pub fn ensure_data_not_pruned_for_checkpoint(
        &self,
        checkpoint_seq: u64,
        tables: &[CommitterTables],
    ) -> IndexerResult<()> {
        if let Some(min_available_cp) = self
            .watermark_cache
            .get_lowest_available_cp_for_tables(tables)
        {
            if (checkpoint_seq as i64) < min_available_cp {
                return Err(IndexerError::DataPruned(format!(
                    "checkpoint {checkpoint_seq} has been pruned (min available: {min_available_cp})"
                )));
            }
        }
        Ok(())
    }

    /// Ensures that the specified tables have data available for the given
    /// transaction. Returns an error if any of the tables have been pruned
    /// for this transaction.
    pub fn ensure_data_not_pruned_for_tx(
        &self,
        tx_seq: i64,
        tables: &[CommitterTables],
    ) -> IndexerResult<()> {
        if let Some(min_available_tx) = self
            .watermark_cache
            .get_lowest_available_tx_for_tables(tables)
        {
            if tx_seq < min_available_tx {
                return Err(IndexerError::DataPruned(format!(
                    "transaction {tx_seq} has been pruned (min available: {min_available_tx})"
                )));
            }
        }
        Ok(())
    }

    pub async fn spawn_blocking<F, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(Self) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let this = self.clone();
        let current_span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            mark_in_blocking_pool();
            let _guard = current_span.enter();
            f(this)
        })
        .await
        .expect("propagate any panics")
    }

    pub fn get_pool(&self) -> ConnectionPool {
        self.pool.clone()
    }
}

// Impl for reading data from the DB
impl IndexerReader {
    fn get_object_from_db(
        &self,
        object_id: &ObjectID,
        version: Option<VersionNumber>,
    ) -> Result<Option<StoredObject>, IndexerError> {
        let object_id = object_id.as_bytes();

        let stored_object = run_query!(&self.pool, |conn| {
            if let Some(version) = version {
                objects::dsl::objects
                    .filter(objects::dsl::object_id.eq(object_id))
                    .filter(objects::dsl::object_version.eq(version.as_u64() as i64))
                    .first::<StoredObject>(conn)
                    .optional()
            } else {
                objects::dsl::objects
                    .filter(objects::dsl::object_id.eq(object_id))
                    .first::<StoredObject>(conn)
                    .optional()
            }
        })?;
        Ok(stored_object)
    }

    fn get_object(
        &self,
        object_id: &ObjectID,
        version: Option<VersionNumber>,
    ) -> Result<Option<Object>, IndexerError> {
        let Some(stored_package) = self.get_object_from_db(object_id, version)? else {
            return Ok(None);
        };

        let object = stored_package.try_into()?;
        Ok(Some(object))
    }

    pub async fn get_object_in_blocking_task(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<Object>, IndexerError> {
        self.spawn_blocking(move |this| this.get_object(&object_id, None))
            .await
    }

    pub async fn get_object_read_in_blocking_task(
        &self,
        object_id: ObjectID,
    ) -> Result<ObjectRead, IndexerError> {
        let stored_object = self
            .spawn_blocking(move |this| this.get_object_raw(object_id))
            .await?;

        if let Some(object) = stored_object {
            object.try_into_object_read(&self.package_resolver).await
        } else {
            Ok(ObjectRead::NotExists(object_id))
        }
    }

    fn get_object_raw(&self, object_id: ObjectID) -> Result<Option<StoredObject>, IndexerError> {
        let id = object_id.as_bytes();
        let stored_object = run_query!(&self.pool, |conn| {
            objects::dsl::objects
                .filter(objects::dsl::object_id.eq(id))
                .first::<StoredObject>(conn)
                .optional()
        })?;
        Ok(stored_object)
    }

    /// Fetches a past object by its ID and version.
    ///
    /// - If `before_version` is `false`, it looks for the exact version.
    /// - If `true`, it finds the latest version before the given one.
    ///
    /// Searches the requested object version and checkpoint sequence number
    /// in `objects_version` and fetches the requested object from
    /// `objects_history`.
    ///
    /// Returns [`IndexerError::DataPruned`] if the object version exists but
    /// history was pruned
    pub(crate) async fn get_past_object_read(
        &self,
        object_id: ObjectID,
        object_version: SequenceNumber,
        before_version: bool,
    ) -> IndexerResult<PastObjectRead> {
        let object_version_num = object_version.as_u64() as i64;

        // Query objects_version to find the requested version and relevant
        // checkpoint sequence number considering the `before_version` flag.
        let object_version_info = self
            .db()
            .get_object_version(object_id, object_version, before_version)
            .await?;

        let Some(object_version_info) = object_version_info else {
            // Check if the object ever existed.
            let latest_existing_version =
                self.db().latest_existing_object_version(object_id).await?;

            return match latest_existing_version {
                Some(latest) if object_version_num > latest => Ok(PastObjectRead::VersionTooHigh {
                    object_id,
                    asked_version: object_version,
                    latest_version: SequenceNumber::from(latest as u64),
                }),
                Some(_) => Ok(PastObjectRead::VersionNotFound(object_id, object_version)),
                None => Ok(PastObjectRead::ObjectNotExists(object_id)),
            };
        };

        // query objects_history for the object with the requested version.
        let history_object = self
            .db()
            .get_stored_history_object(
                object_id,
                object_version_info.object_version,
                object_version_info.cp_sequence_number,
            )
            .await?;

        match history_object {
            Some(obj) => obj.try_into_past_object_read(&self.package_resolver).await,
            None => Err(IndexerError::DataPruned(format!(
                "Object version {} not found in objects_history for object {object_id}",
                object_version_info.object_version
            ))),
        }
    }

    /// Fetches a past object by its ID and version.
    ///
    /// - If `before_version` is `false`, it looks for the exact version.
    /// - If `true`, it finds the latest version before the given one.
    ///
    /// Searches the requested object version and checkpoint sequence number
    /// in `objects_version` and fetches the requested object from
    /// `objects_history`.
    ///
    /// Retrieval order:
    /// 1. Postgres database (`objects_version` + `objects_history`)
    /// 2. Historical fallback storage (if enabled)
    pub(crate) async fn get_past_object_read_with_fallback(
        &self,
        object_id: ObjectID,
        object_version: SequenceNumber,
        before_version: bool,
    ) -> IndexerResult<PastObjectRead> {
        let past_object_read_result = self
            .get_past_object_read(object_id, object_version, before_version)
            .await;

        let Some(fallback) = self.fallback_reader().filter(|_| {
            matches!(
                past_object_read_result,
                Err(IndexerError::DataPruned(_)) | Ok(PastObjectRead::ObjectNotExists(_))
            )
        }) else {
            return past_object_read_result;
        };

        let Some(obj) = fallback
            .objects(&[(object_id, object_version)], before_version)
            .await?
            .pop()
            .flatten()
        else {
            return Ok(PastObjectRead::VersionNotFound(object_id, object_version));
        };

        // Note: We use `StoredObject.try_into_object_read` here instead of
        // `StoredHistoryObject.try_into_past_object_read` because the fallback
        // storage returns `StoredObject`. Both methods share the same logic for
        // resolving the MoveStructLayout via `package_resolver.type_layout()`.
        // The key difference is that `try_into_past_object_read` also handles
        // the `WrappedOrDeleted` status, which for this iteration, we handle explicitly
        // as a `VersionNotFound`.
        match obj.try_into_object_read(&self.package_resolver).await? {
            ObjectRead::NotExists(_) | ObjectRead::Deleted(_) => {
                Ok(PastObjectRead::VersionNotFound(object_id, object_version))
            }
            ObjectRead::Exists(obj_ref, object, layout) => {
                Ok(PastObjectRead::VersionFound(obj_ref, object, layout))
            }
        }
    }

    pub async fn get_package(&self, package_id: ObjectID) -> Result<Package, IndexerError> {
        let store = self.package_resolver.package_store();
        let pkg = store
            .fetch(package_id.into())
            .await
            .map_err(|e| {
                IndexerError::PostgresRead(format!(
                    "Fail to fetch package from package store with error {e:?}"
                ))
            })?
            .as_ref()
            .clone();
        Ok(pkg)
    }

    pub fn get_epoch_info_from_db(
        &self,
        epoch: Option<EpochId>,
    ) -> Result<Option<StoredEpochInfo>, IndexerError> {
        let stored_epoch = run_query!(&self.pool, |conn| {
            if let Some(epoch) = epoch {
                epochs::dsl::epochs
                    .filter(epochs::epoch.eq(epoch as i64))
                    .first::<StoredEpochInfo>(conn)
                    .optional()
            } else {
                epochs::dsl::epochs
                    .order_by(epochs::epoch.desc())
                    .first::<StoredEpochInfo>(conn)
                    .optional()
            }
        })?;

        Ok(stored_epoch)
    }

    pub fn get_latest_epoch_info_from_db(&self) -> Result<StoredEpochInfo, IndexerError> {
        let stored_epoch = run_query!(&self.pool, |conn| {
            epochs::dsl::epochs
                .order_by(epochs::epoch.desc())
                .first::<StoredEpochInfo>(conn)
        })?;

        Ok(stored_epoch)
    }

    pub fn get_epoch_info(
        &self,
        epoch: Option<EpochId>,
    ) -> Result<Option<EpochInfo>, IndexerError> {
        let stored_epoch = self.get_epoch_info_from_db(epoch)?;

        let stored_epoch = match stored_epoch {
            Some(stored_epoch) => stored_epoch,
            None => return Ok(None),
        };

        let epoch_info = EpochInfo::try_from(stored_epoch)?;
        Ok(Some(epoch_info))
    }

    fn get_epochs_from_db(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> Result<Vec<StoredEpochInfo>, IndexerError> {
        run_query!(&self.pool, |conn| {
            let mut boxed_query = epochs::table.into_boxed();
            if let Some(cursor) = cursor {
                if descending_order {
                    boxed_query = boxed_query.filter(epochs::epoch.lt(cursor as i64));
                } else {
                    boxed_query = boxed_query.filter(epochs::epoch.gt(cursor as i64));
                }
            }
            if descending_order {
                boxed_query = boxed_query.order_by(epochs::epoch.desc());
            } else {
                boxed_query = boxed_query.order_by(epochs::epoch.asc());
            }

            boxed_query.limit(limit as i64).load(conn)
        })
    }

    pub fn get_epochs(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> Result<Vec<EpochInfo>, IndexerError> {
        self.get_epochs_from_db(cursor, limit, descending_order)?
            .into_iter()
            .map(EpochInfo::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn get_latest_iota_system_state(&self) -> Result<IotaSystemStateSummary, IndexerError> {
        let system_state: IotaSystemStateSummary =
            iota_types::iota_system_state::get_iota_system_state(self)?
                .into_iota_system_state_summary();
        Ok(system_state)
    }

    /// Retrieve the system state data for the given epoch. If no epoch is
    /// given, it will retrieve the latest epoch's data and return the
    /// system state. System state of the an epoch is written at the end of
    /// the epoch, so system state of the current epoch is empty until the
    /// epoch ends. You can call `get_latest_iota_system_state` for current
    /// epoch instead.
    pub fn get_epoch_iota_system_state(
        &self,
        epoch: Option<EpochId>,
    ) -> Result<IotaSystemStateSummary, IndexerError> {
        let stored_epoch = self.get_epoch_info_from_db(epoch)?;
        let stored_epoch = match stored_epoch {
            Some(stored_epoch) => stored_epoch,
            None => return Err(IndexerError::InvalidArgument("Invalid epoch".into())),
        };

        Ok(StoredSystemState::try_from(&stored_epoch)?.into())
    }

    pub async fn get_chain_identifier_in_blocking_task(
        &self,
    ) -> Result<ChainIdentifier, IndexerError> {
        self.spawn_blocking(|this| this.get_chain_identifier())
            .await
    }

    pub fn get_chain_identifier(&self) -> Result<ChainIdentifier, IndexerError> {
        let stored_chain_identifier = run_query!(&self.pool, |conn| {
            chain_identifier::dsl::chain_identifier
                .first::<StoredChainIdentifier>(conn)
                .optional()
        })?
        .ok_or(IndexerError::PostgresRead(
            "chain identifier not found".to_string(),
        ))?;

        let checkpoint_digest = CheckpointDigest::from_bytes(
            stored_chain_identifier.checkpoint_digest,
        )
        .map_err(|e| {
            IndexerError::PersistentStorageDataCorruption(format!(
                "failed to decode chain identifier with err: {e:?}"
            ))
        })?;

        Ok(checkpoint_digest.into())
    }

    pub fn get_latest_checkpoint_from_db(&self) -> Result<StoredCheckpoint, IndexerError> {
        let stored_checkpoint = run_query!(&self.pool, |conn| {
            checkpoints::dsl::checkpoints
                .order_by(checkpoints::sequence_number.desc())
                .first::<StoredCheckpoint>(conn)
        })?;

        Ok(stored_checkpoint)
    }

    /// Fetches a single checkpoint by either its sequence number or digest.
    ///
    /// Retrieval order:
    /// 1. Postgres database
    /// 2. Historical fallback storage (if enabled)
    pub async fn get_checkpoint_with_fallback(
        &self,
        checkpoint_id: CheckpointId,
    ) -> IndexerResult<Option<iota_json_rpc_types::Checkpoint>> {
        let stored_checkpoint = match self.db().get_checkpoint(checkpoint_id).await {
            Ok(res) => res,
            Err(IndexerError::DataPruned(_)) => {
                // Data is pruned, fallback to historical storage
                self.fallback_reader()
                    .ok_or_else(|| {
                        IndexerError::DataPruned(format!(
                            "checkpoint {checkpoint_id:?} has been pruned and fallback storage is not available"
                        ))
                    })?
                    .checkpoint(checkpoint_id)
                    .await?
                    .ok_or_else(|| {
                        IndexerError::DataPruned(format!(
                            "checkpoint {checkpoint_id:?} has been pruned and is not available in fallback storage"
                        ))
                    })
                    .map(Some)?
            }
            Err(e) => return Err(e),
        };

        stored_checkpoint
            .map(iota_json_rpc_types::Checkpoint::try_from)
            .transpose()
    }

    pub fn get_latest_checkpoint(&self) -> Result<iota_json_rpc_types::Checkpoint, IndexerError> {
        let stored_checkpoint = self.get_latest_checkpoint_from_db()?;

        iota_json_rpc_types::Checkpoint::try_from(stored_checkpoint)
    }

    pub async fn get_latest_checkpoint_timestamp_ms_in_blocking_task(
        &self,
    ) -> Result<u64, IndexerError> {
        self.spawn_blocking(|this| this.get_latest_checkpoint_timestamp_ms())
            .await
    }

    pub fn get_latest_checkpoint_timestamp_ms(&self) -> Result<u64, IndexerError> {
        Ok(self.get_latest_checkpoint()?.timestamp_ms)
    }

    /// Determines whether the fallback should be used to fetch more
    /// checkpoints in case of data being pruned.
    fn should_fetch_from_fallback(
        cursor: Option<u64>,
        descending_order: bool,
        limit: usize,
        db_response: &[iota_json_rpc_types::Checkpoint],
    ) -> bool {
        match (cursor, descending_order) {
            // pruning always removes from the lowest checkpoint upwards, so no gaps.
            // If genesis (checkpoint 0) is present, data is intact.
            (None, false) => db_response
                .first()
                .is_none_or(|chk| chk.sequence_number != 0),

            // for descending, cursor just sets an upper bound. DB returns the highest checkpoint
            // available. If we got fewer than limit, some data was pruned.
            (None, true) | (Some(_), true) => {
                db_response.len() != limit
                    && db_response.last().is_some_and(|cp| cp.sequence_number != 0)
            }

            // if first checkpoint matches cursor + 1, data is contiguous from that point.
            (Some(c), false) => db_response
                .first()
                .is_none_or(|chk| chk.sequence_number != c + 1),
        }
    }

    /// Fetches checkpoints from the indexer storage.
    ///
    /// Retrieval order:
    /// 1. Postgres database
    /// 2. Historical fallback storage (if enabled)
    ///
    /// Returns [`IndexerError::DataPruned`] if the requested checkpoint range
    /// is not available and fallback is not enabled.
    pub async fn get_checkpoints_with_fallback(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> Result<Vec<iota_json_rpc_types::Checkpoint>, IndexerError> {
        let checkpoints = self
            .db()
            .get_checkpoints(cursor, limit, descending_order)
            .await?
            .into_iter()
            .map(iota_json_rpc_types::Checkpoint::try_from)
            .collect::<IndexerResult<Vec<_>>>()?;

        if !Self::should_fetch_from_fallback(cursor, descending_order, limit, &checkpoints) {
            return Ok(checkpoints);
        }

        // resolve the expected range of checkpoint sequence numbers
        let checkpoints_keys: Vec<CheckpointSequenceNumber> = match (cursor, descending_order) {
            // ascending from 0: expect [0, 1, ..., limit-1].
            (None, false) => (0..limit as u64).collect(),

            // ascending from cursor+1: expect [c+1, ..., c+limit]
            (Some(c), false) => (c + 1..=c.saturating_add(limit as u64)).collect(),

            // descending from cursor-1: expect [c-1, c-2, ..., c-limit].
            (Some(c), true) => {
                // cursor can be greater than the latest checkpoint, need to cap it.
                let c = checkpoints
                    .first()
                    .map(|latest_checkpoint| c.min(latest_checkpoint.sequence_number + 1))
                    .unwrap_or(c);

                (c.saturating_sub(limit as u64)..c).rev().collect()
            }

            // descending from DB's latest: expect [latest, ..., latest-limit+1].
            (None, true) => {
                let Some(latest_checkpoint) = checkpoints.first() else {
                    // checkpoints not synced yet.
                    return Ok(vec![]);
                };
                let start = latest_checkpoint
                    .sequence_number
                    .saturating_sub(limit as u64 - 1);
                (start..=latest_checkpoint.sequence_number).rev().collect()
            }
        };

        // fallback to historical storage
        let Some(fallback) = self.fallback_reader() else {
            return Err(IndexerError::DataPruned(
                "requested checkpoint range not available".into(),
            ));
        };

        fallback
            .checkpoints(checkpoints_keys)
            .await?
            .into_iter()
            .flatten()
            .map(iota_json_rpc_types::Checkpoint::try_from)
            .collect()
    }

    /// Fetches multiple transactions from the database.
    ///
    ///  Retrieval order:
    /// 1. Checkpointed data (finalized transactions)
    /// 2. Optimistic data (pending transactions not yet checkpointed)
    pub(crate) async fn multi_get_transactions(
        &self,
        digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<StoredTransaction>> {
        let digests: Vec<Vec<u8>> = digests.iter().map(|d| d.inner().to_vec()).collect();
        let checkpointed_txs = self
            .db()
            .get_checkpointed_transactions(digests.clone())
            .await?;

        if checkpointed_txs.len() == digests.len() {
            return Ok(checkpointed_txs);
        }

        let missing_digests = Self::check_for_missing_tx_digests(&digests, &checkpointed_txs);
        let optimistic_txs = self
            .db()
            .get_optimistic_transactions(missing_digests)
            .await?;

        Ok(checkpointed_txs
            .into_iter()
            .chain(optimistic_txs.into_iter().map(Into::into))
            .collect::<Vec<StoredTransaction>>())
    }

    /// Fetches multiple transactions from the indexer storage.
    ///
    /// Retrieval order:
    /// 1. Checkpointed data (finalized transactions)
    /// 2. Optimistic data (pending transactions not yet checkpointed)
    /// 3. Historical fallback storage (if enabled)
    async fn multi_get_transactions_with_fallback(
        &self,
        digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<StoredTransaction>> {
        let fetched_transactions = self.multi_get_transactions(digests).await?;

        // fallback to historical storage
        let Some(fallback) = self
            .fallback_reader()
            // As for now we don't have a way to identify if the user requested pruned or invalid
            // transaction digests. As a measure, we check if the number of requested transactions
            // matches the number of fetched transactions. In case of missing transactions,
            // if fallback is enabled, we use it to fetch the missing ones.
            .filter(|_| fetched_transactions.len() != digests.len())
        else {
            // return data from database.
            return Ok(fetched_transactions);
        };

        let digests: Vec<Vec<u8>> = digests.iter().map(|d| d.inner().to_vec()).collect();
        let missing_digests = Self::check_for_missing_tx_digests(&digests, &fetched_transactions)
            .iter()
            .map(|digest| {
                TransactionDigest::from_bytes(digest.as_slice()).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "can't convert {digest:?} as tx_digest. Error: {e}",
                    ))
                })
            })
            .collect::<IndexerResult<Vec<TransactionDigest>>>()?;

        let historical_transactions = fallback
            .transactions(&missing_digests)
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<StoredTransaction>>();

        Ok(fetched_transactions
            .into_iter()
            .chain(historical_transactions)
            .collect())
    }

    /// Checks for missing transaction digests in the fetched transactions.
    fn check_for_missing_tx_digests(
        requested_digests: &[Vec<u8>],
        fetched_txs: &[StoredTransaction],
    ) -> Vec<Vec<u8>> {
        let fetched_txs_digests_set = fetched_txs
            .iter()
            .map(|tx| &tx.transaction_digest)
            .collect::<HashSet<&Vec<u8>>>();
        requested_digests
            .iter()
            .filter(|digest| !fetched_txs_digests_set.contains(digest))
            .cloned()
            .collect::<Vec<Vec<u8>>>()
    }

    /// This method tries to transform [`StoredTransaction`] values
    /// into transaction blocks, without any other modification.
    pub(crate) async fn stored_transaction_to_transaction_block(
        &self,
        stored_txes: Vec<StoredTransaction>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        let mut tx_block_responses_futures = vec![];
        for stored_tx in stored_txes {
            let options_clone = options.clone();
            let package_resolver = self.package_resolver.clone();
            tx_block_responses_futures.push(tokio::task::spawn(async move {
                stored_tx
                    .try_into_iota_transaction_block_response(options_clone, &package_resolver)
                    .await
            }));
        }

        let tx_blocks = futures::future::join_all(tx_block_responses_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("failed to join all tx block futures: {e}"))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("failed to collect tx block futures: {e}"))?;
        Ok(tx_blocks)
    }

    fn multi_get_transactions_with_sequence_numbers(
        &self,
        tx_sequence_numbers: Vec<i64>,
        // Some(true) for desc, Some(false) for asc, None for undefined order
        is_descending: Option<bool>,
    ) -> Result<Vec<StoredTransaction>, IndexerError> {
        let mut query = transactions::table
            .filter(transactions::tx_sequence_number.eq_any(tx_sequence_numbers))
            .into_boxed();
        match is_descending {
            Some(true) => {
                query = query.order(transactions::dsl::tx_sequence_number.desc());
            }
            Some(false) => {
                query = query.order(transactions::dsl::tx_sequence_number.asc());
            }
            None => (),
        }
        run_query!(&self.pool, |conn| query.load::<StoredTransaction>(conn))
    }

    pub fn multi_get_transactions_by_sequence_numbers_range(
        &self,
        min_seq: i64,
        max_seq: i64,
    ) -> Result<Vec<StoredTransaction>, IndexerError> {
        use crate::schema::transactions::dsl as txdsl;
        let query = txdsl::transactions
            .filter(txdsl::tx_sequence_number.ge(min_seq))
            .filter(txdsl::tx_sequence_number.le(max_seq))
            .order(txdsl::tx_sequence_number.asc())
            .into_boxed();
        run_query!(&self.pool, |conn| query.load::<StoredTransaction>(conn))
    }

    pub async fn get_owned_objects_in_blocking_task(
        &self,
        address: IotaAddress,
        filter: Option<IotaObjectDataFilter>,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        self.spawn_blocking(move |this| this.get_owned_objects_impl(address, filter, cursor, limit))
            .await
    }

    fn get_owned_objects_impl(
        &self,
        address: IotaAddress,
        filter: Option<IotaObjectDataFilter>,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        run_query!(&self.pool, |conn| {
            let mut query = objects::dsl::objects
                .filter(objects::dsl::owner_type.eq(OwnerType::Address as i16))
                .filter(objects::dsl::owner_id.eq(address.as_bytes()))
                .order(objects::dsl::object_id.asc())
                .limit(limit as i64)
                .into_boxed();
            if let Some(filter) = filter {
                match filter {
                    IotaObjectDataFilter::StructType(struct_tag) => {
                        let object_type =
                            struct_tag.to_canonical_string(/* with_prefix */ true);
                        query = query.filter(objects::object_type.like(format!("{object_type}%")));
                    }
                    IotaObjectDataFilter::MatchAny(filters) => {
                        let mut condition = "(".to_string();
                        for (i, filter) in filters.iter().enumerate() {
                            if let IotaObjectDataFilter::StructType(struct_tag) = filter {
                                let object_type =
                                    struct_tag.to_canonical_string(/* with_prefix */ true);
                                if i == 0 {
                                    condition +=
                                        format!("objects.object_type LIKE '{object_type}%'")
                                            .as_str();
                                } else {
                                    condition +=
                                        format!(" OR objects.object_type LIKE '{object_type}%'")
                                            .as_str();
                                }
                            } else {
                                return Err(IndexerError::InvalidArgument(
                                    "Invalid filter type. Only struct, MatchAny and MatchNone of struct filters are supported.".into(),
                                ));
                            }
                        }
                        condition += ")";
                        query = query.filter(sql::<Bool>(&condition));
                    }
                    IotaObjectDataFilter::MatchNone(filters) => {
                        for filter in filters {
                            if let IotaObjectDataFilter::StructType(struct_tag) = filter {
                                let object_type =
                                    struct_tag.to_canonical_string(/* with_prefix */ true);
                                query = query.filter(
                                    objects::object_type.not_like(format!("{object_type}%")),
                                );
                            } else {
                                return Err(IndexerError::InvalidArgument(
                                    "Invalid filter type. Only struct, MatchAny and MatchNone of struct filters are supported.".into(),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(IndexerError::InvalidArgument(
                            "Invalid filter type. Only struct, MatchAny and MatchNone of struct filters are supported.".into(),
                        ));
                    }
                }
            }

            if let Some(object_cursor) = cursor {
                query = query.filter(objects::dsl::object_id.gt(object_cursor.as_bytes().to_vec()));
            }

            query
                .load::<StoredObject>(conn)
                .map_err(|e| IndexerError::PostgresRead(e.to_string()))
        })
    }

    fn get_singleton_object(&self, struct_tag: StructTag) -> Result<Option<Object>, IndexerError> {
        let object_type = struct_tag.to_canonical_string(/* with_prefix */ true);

        run_query!(&self.pool, |conn| {
            let object = match objects::dsl::objects
                .filter(objects::object_type_package.eq(struct_tag.address().as_bytes().to_vec()))
                .filter(objects::object_type_module.eq(struct_tag.module().to_string()))
                .filter(objects::object_type_name.eq(struct_tag.name().to_string()))
                .filter(objects::object_type.eq(object_type))
                .first::<StoredObject>(conn)
                .optional()
                .map_err(|e| IndexerError::PostgresRead(e.to_string()))?
            {
                Some(object) => object,
                None => return Ok::<Option<Object>, IndexerError>(None),
            }
            .try_into()?;
            Ok(Some(object))
        })
    }

    pub async fn multi_get_objects_in_blocking_task(
        &self,
        object_ids: Vec<ObjectID>,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        self.spawn_blocking(move |this| this.multi_get_objects_impl(object_ids))
            .await
    }

    fn multi_get_objects_impl(
        &self,
        object_ids: Vec<ObjectID>,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        let object_ids = object_ids.iter().map(|id| id.as_bytes()).collect_vec();
        run_query!(&self.pool, |conn| {
            objects::dsl::objects
                .filter(objects::object_id.eq_any(object_ids))
                .load::<StoredObject>(conn)
        })
    }

    /// Checks whether all input objects exist at the requested versions and
    /// are finalized.
    pub async fn check_input_objects_in_blocking_task(
        &self,
        object_keys: Vec<(ObjectID, SequenceNumber)>,
    ) -> Result<InputObjectsStatus, IndexerError> {
        self.spawn_blocking(move |this| this.check_input_objects(object_keys))
            .await
    }

    fn check_input_objects(
        &self,
        object_keys: Vec<(ObjectID, SequenceNumber)>,
    ) -> Result<InputObjectsStatus, IndexerError> {
        if object_keys.is_empty() {
            return Ok(InputObjectsStatus::Ready);
        }

        let values_clause = object_keys
            .iter()
            .map(|(id, version)| {
                format!(
                    "('\\x{}'::bytea, {}::bigint)",
                    Hex::encode(id.into_bytes()),
                    version.as_u64()
                )
            })
            .join(", ");

        // Single query that returns a tri-state:
        // TRUE  — all objects match exact versions and are finalized
        // FALSE — at least one object has been superseded (newer version)
        // NULL  — not ready yet (missing, older version, or not finalized)
        let query = format!(
            "WITH \
               input_objects(id, version) AS (VALUES {values_clause}), \
               max_chk AS (SELECT COALESCE(MAX(sequence_number), -1) AS max_sn FROM checkpoints), \
               matches AS ( \
                 SELECT \
                   i.version AS input_version, \
                   o.object_version AS actual_version, \
                   o.finalized_in_cp \
                 FROM input_objects i \
                 LEFT JOIN objects o ON o.object_id = i.id \
               ) \
             SELECT CASE \
               WHEN COUNT(*) FILTER (WHERE actual_version > input_version) > 0 THEN FALSE \
               WHEN COUNT(*) FILTER (WHERE actual_version IS NULL) > 0 THEN NULL \
               WHEN COUNT(*) FILTER (WHERE actual_version < input_version) > 0 THEN NULL \
               WHEN COUNT(*) FILTER (WHERE finalized_in_cp IS NOT NULL \
                 AND finalized_in_cp > (SELECT max_sn FROM max_chk)) > 0 THEN NULL \
               ELSE TRUE \
             END AS result FROM matches"
        );

        #[derive(QueryableByName)]
        struct TriState {
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Bool>)]
            result: Option<bool>,
        }

        run_query!(&self.pool, |conn| {
            diesel::sql_query(query)
                .get_result::<TriState>(conn)
                .map(|r| match r.result {
                    Some(true) => InputObjectsStatus::Ready,
                    Some(false) => InputObjectsStatus::Superseded,
                    None => InputObjectsStatus::Pending,
                })
        })
    }

    pub async fn query_transaction_blocks_in_blocking_task(
        &self,
        filter: Option<TransactionFilter>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        self.query_transaction_blocks_impl_with_checkpointed_data_only(
            filter.map(TransactionFilterKind::V1),
            options,
            cursor,
            limit,
            is_descending,
        )
        .await
    }

    pub async fn query_transaction_blocks_in_blocking_task_v2(
        &self,
        filter: Option<TransactionFilterV2>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        self.query_transaction_blocks_impl_with_checkpointed_data_only(
            filter.map(TransactionFilterKind::V2),
            options,
            cursor,
            limit,
            is_descending,
        )
        .await
    }

    async fn query_transactions_by_checkpoint_seq_with_fallback(
        &self,
        checkpoint_seq: u64,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        let db_res = self
            .db()
            .query_transactions_by_checkpoint_seq(checkpoint_seq, cursor, limit, is_descending)
            .await;
        let stored_txs = if let (Err(IndexerError::DataPruned(err)), Some(kv_reader)) =
            (db_res.as_ref(), self.fallback_reader())
        {
            kv_reader
                .checkpoint_transactions(cursor, checkpoint_seq, limit, is_descending)
                .await
                .context(&format!("fallback triggered by {err}"))?
        } else {
            db_res?
        };
        self.stored_transaction_to_transaction_block(stored_txs, options)
            .await
    }

    async fn query_transaction_blocks_impl_with_checkpointed_data_only(
        &self,
        filter: Option<TransactionFilterKind>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        if let Some(TransactionFilterKind::V1(TransactionFilter::Checkpoint(seq)))
        | Some(TransactionFilterKind::V2(TransactionFilterV2::Checkpoint(seq))) = filter
        {
            return self
                .query_transactions_by_checkpoint_seq_with_fallback(
                    seq,
                    cursor,
                    limit,
                    is_descending,
                    options,
                )
                .await;
        };

        // All transaction-related tables that could be used by any filter
        let tx_tables = [
            CommitterTables::Transactions,
            CommitterTables::TxCallsFun,
            CommitterTables::TxCallsMod,
            CommitterTables::TxCallsPkg,
            CommitterTables::TxInputObjects,
            CommitterTables::TxChangedObjects,
            CommitterTables::TxWrappedOrDeletedObjects,
            CommitterTables::TxSenders,
            CommitterTables::TxRecipients,
            CommitterTables::TxKinds,
        ];
        let min_available_tx = self
            .watermark_cache
            .get_lowest_available_tx_for_tables(&tx_tables)
            .unwrap_or(0);

        let cursor_tx_seq = if let Some(cursor) = cursor {
            let tx_seq = self
                .db()
                .resolve_cursor_tx_digest_to_seq_num(cursor)
                .await?;
            self.ensure_data_not_pruned_for_tx(tx_seq, &tx_tables)?;
            Some(tx_seq)
        } else {
            None
        };
        let cursor_clause = if let Some(cursor_tx_seq) = cursor_tx_seq {
            if is_descending {
                format!("AND {TX_SEQUENCE_NUMBER_STR} < {cursor_tx_seq}")
            } else {
                format!("AND {TX_SEQUENCE_NUMBER_STR} > {cursor_tx_seq}")
            }
        } else {
            "".to_string()
        };
        let order_str = if is_descending { "DESC" } else { "ASC" };

        let (table_name, main_where_clause) = match filter {
            // Processed above
            Some(TransactionFilterKind::V1(TransactionFilter::Checkpoint(_)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::Checkpoint(_))) => {
                unreachable!("handled in earlier match statement")
            }
            // FIXME: sanitize module & function
            Some(TransactionFilterKind::V1(TransactionFilter::MoveFunction {
                package,
                module,
                function,
            }))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::MoveFunction {
                package,
                module,
                function,
            })) => {
                let package = Hex::encode(package.as_bytes());
                match (module, function) {
                    (Some(module), Some(function)) => (
                        "tx_calls_fun".into(),
                        format!(
                            "package = '\\x{package}'::bytea AND module = '{module}' AND func = '{function}'"
                        ),
                    ),
                    (Some(module), None) => (
                        "tx_calls_mod".into(),
                        format!("package = '\\x{package}'::bytea AND module = '{module}'"),
                    ),
                    (None, Some(_)) => {
                        return Err(IndexerError::InvalidArgument(
                            "Function cannot be present without Module.".into(),
                        ));
                    }
                    (None, None) => (
                        "tx_calls_pkg".into(),
                        format!("package = '\\x{package}'::bytea"),
                    ),
                }
            }
            Some(TransactionFilterKind::V1(TransactionFilter::InputObject(object_id)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::InputObject(object_id))) => {
                let object_id = Hex::encode(object_id.as_bytes());
                (
                    "tx_input_objects".into(),
                    format!("object_id = '\\x{object_id}'::bytea"),
                )
            }
            Some(TransactionFilterKind::V1(TransactionFilter::ChangedObject(object_id)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::ChangedObject(object_id))) => {
                let object_id = Hex::encode(object_id.as_bytes());
                (
                    "tx_changed_objects".into(),
                    format!("object_id = '\\x{object_id}'::bytea"),
                )
            }
            Some(TransactionFilterKind::V2(TransactionFilterV2::WrappedOrDeletedObject(
                object_id,
            ))) => {
                let object_id = Hex::encode(object_id.as_bytes());
                (
                    "tx_wrapped_or_deleted_objects".into(),
                    format!("object_id = '\\x{object_id}'::bytea"),
                )
            }
            Some(TransactionFilterKind::V1(TransactionFilter::FromAddress(from_address)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::FromAddress(from_address))) => {
                let from_address = Hex::encode(from_address.as_bytes());
                (
                    "tx_senders".into(),
                    format!("sender = '\\x{from_address}'::bytea"),
                )
            }
            Some(TransactionFilterKind::V1(TransactionFilter::ToAddress(to_address)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::ToAddress(to_address))) => {
                let to_address = Hex::encode(to_address.as_bytes());
                (
                    "tx_recipients".into(),
                    format!("recipient = '\\x{to_address}'::bytea"),
                )
            }
            Some(TransactionFilterKind::V1(TransactionFilter::FromAndToAddress { from, to }))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::FromAndToAddress { from, to })) =>
            {
                let from_address = Hex::encode(from.as_bytes());
                let to_address = Hex::encode(to.as_bytes());
                // Need to remove ambiguities for tx_sequence_number column
                let cursor_clause = if let Some(cursor_tx_seq) = cursor_tx_seq {
                    if is_descending {
                        format!("AND tx_senders.{TX_SEQUENCE_NUMBER_STR} < {cursor_tx_seq}")
                    } else {
                        format!("AND tx_senders.{TX_SEQUENCE_NUMBER_STR} > {cursor_tx_seq}")
                    }
                } else {
                    "".to_string()
                };
                let inner_query = format!(
                    "(SELECT tx_senders.{TX_SEQUENCE_NUMBER_STR} \
                    FROM tx_senders \
                    JOIN tx_recipients \
                    ON tx_senders.{TX_SEQUENCE_NUMBER_STR} = tx_recipients.{TX_SEQUENCE_NUMBER_STR} \
                    WHERE tx_senders.sender = '\\x{from_address}'::BYTEA \
                    AND tx_recipients.recipient = '\\x{to_address}'::BYTEA \
                    {cursor_clause} \
                    ORDER BY {TX_SEQUENCE_NUMBER_STR} {order_str} \
                    LIMIT {limit}) AS inner_query
                    ",
                );
                (inner_query, "1 = 1".into())
            }
            Some(TransactionFilterKind::V1(TransactionFilter::FromOrToAddress { addr }))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::FromOrToAddress { addr })) => {
                let address = Hex::encode(addr.as_bytes());
                let inner_query = format!(
                    "( \
                        ( \
                            SELECT {TX_SEQUENCE_NUMBER_STR} FROM tx_senders \
                            WHERE sender = '\\x{address}'::BYTEA {cursor_clause} \
                            ORDER BY {TX_SEQUENCE_NUMBER_STR} {order_str} \
                            LIMIT {limit} \
                        ) \
                        UNION \
                        ( \
                            SELECT {TX_SEQUENCE_NUMBER_STR} FROM tx_recipients \
                            WHERE recipient = '\\x{address}'::BYTEA {cursor_clause} \
                            ORDER BY {TX_SEQUENCE_NUMBER_STR} {order_str} \
                            LIMIT {limit} \
                        ) \
                    ) AS combined",
                );
                (inner_query, "1 = 1".into())
            }
            Some(TransactionFilterKind::V1(TransactionFilter::TransactionKind(kind)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::TransactionKind(kind))) => {
                // The `SystemTransaction` variant can be used to filter for all types of system
                // transactions.
                if kind == IotaTransactionKind::SystemTransaction {
                    ("tx_kinds".into(), "tx_kind != 1".to_string())
                } else {
                    ("tx_kinds".into(), format!("tx_kind = {}", kind as u8))
                }
            }
            Some(TransactionFilterKind::V1(TransactionFilter::TransactionKindIn(kind_vec)))
            | Some(TransactionFilterKind::V2(TransactionFilterV2::TransactionKindIn(kind_vec))) => {
                if kind_vec.is_empty() {
                    return Err(IndexerError::InvalidArgument(
                        "no transaction kind provided".into(),
                    ));
                }

                let mut has_system_transaction = false;
                let mut has_programmable_transaction = false;
                let mut other_kinds = HashSet::new();

                for kind in kind_vec.iter() {
                    match kind {
                        IotaTransactionKind::SystemTransaction => has_system_transaction = true,
                        IotaTransactionKind::ProgrammableTransaction => {
                            has_programmable_transaction = true
                        }
                        other => {
                            other_kinds.insert(*other as u8);
                        }
                    }
                }

                let query = if has_system_transaction {
                    // Case: If `SystemTransaction` is present but `ProgrammableTransaction` is not,
                    // we need to filter out `ProgrammableTransaction`.
                    if !has_programmable_transaction {
                        "tx_kind != 1".to_string()
                    } else {
                        // No filter applied if both exist
                        "1 = 1".to_string()
                    }
                } else {
                    // Case: `ProgrammableTransaction` is present
                    if has_programmable_transaction {
                        other_kinds.insert(IotaTransactionKind::ProgrammableTransaction as u8);
                    }

                    if other_kinds.is_empty() {
                        // If there's nothing to filter on, return an empty query
                        "1 = 1".to_string()
                    } else {
                        let mut query = String::from("tx_kind IN (");
                        query.push_str(
                            &other_kinds
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                        query.push(')');
                        query
                    }
                };

                ("tx_kinds".into(), query)
            }
            Some(TransactionFilterKind::V2(_)) => {
                return Err(IndexerError::InvalidArgument(
                    "transaction filter is not supported".into(),
                ));
            }
            None => {
                // apply no filter
                ("transactions".into(), "1 = 1".into())
            }
        };

        let query = format!(
            "SELECT {TX_SEQUENCE_NUMBER_STR} FROM {table_name} WHERE ({main_where_clause}) AND {TX_SEQUENCE_NUMBER_STR} >= {min_available_tx} {cursor_clause} ORDER BY {TX_SEQUENCE_NUMBER_STR} {order_str} LIMIT {limit}",
        );

        tracing::debug!("query transaction blocks: {}", query);
        let pool = self.get_pool();
        let tx_sequence_numbers = run_query_async!(&pool, move |conn| {
            diesel::sql_query(query).load::<TxSequenceNumber>(conn)
        })?
        .into_iter()
        .map(|tsn| tsn.tx_sequence_number)
        .collect::<Vec<i64>>();
        self.multi_get_transaction_block_response_by_sequence_numbers_in_blocking_task(
            tx_sequence_numbers,
            options,
            Some(is_descending),
        )
        .await
    }

    async fn multi_get_transaction_block_response_in_blocking_task_impl(
        &self,
        digests: &[TransactionDigest],
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> Result<Vec<iota_json_rpc_types::IotaTransactionBlockResponse>, IndexerError> {
        let stored_txes = self.multi_get_transactions_with_fallback(digests).await?;
        self.stored_transaction_to_transaction_block(stored_txes, options)
            .await
    }

    /// Fetches a single transaction block from the indexer storage.
    ///
    /// Retrieval order:
    /// 1. Postgres database
    /// 2. Historical fallback storage (if enabled)
    pub(crate) async fn get_single_transaction_block_response_with_fallback(
        &self,
        digest: TransactionDigest,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> IndexerResult<Option<IotaTransactionBlockResponse>> {
        let stored_tx = match self.db().get_single_transaction(digest).await? {
            Some(tx) => Some(tx),
            None => {
                // fallback to historical storage
                let Some(fallback) = self.fallback_reader() else {
                    return Ok(None);
                };
                fallback.transactions(&[digest]).await?.pop().flatten()
            }
        };

        let Some(stored_tx) = stored_tx else {
            return Ok(None);
        };

        Ok(Some(
            self.stored_transaction_to_transaction_block(vec![stored_tx], options)
                .await?
                .pop()
                .expect("there should be exactly one response"),
        ))
    }

    async fn multi_get_transaction_block_response_by_sequence_numbers_in_blocking_task(
        &self,
        tx_sequence_numbers: Vec<i64>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        // Some(true) for desc, Some(false) for asc, None for undefined order
        is_descending: Option<bool>,
    ) -> Result<Vec<iota_json_rpc_types::IotaTransactionBlockResponse>, IndexerError> {
        let stored_txes: Vec<StoredTransaction> = self
            .spawn_blocking(move |this| {
                this.multi_get_transactions_with_sequence_numbers(
                    tx_sequence_numbers,
                    is_descending,
                )
            })
            .await?;
        self.stored_transaction_to_transaction_block(stored_txes, options)
            .await
    }

    pub async fn multi_get_transaction_block_response_in_blocking_task(
        &self,
        digests: Vec<TransactionDigest>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> Result<Vec<iota_json_rpc_types::IotaTransactionBlockResponse>, IndexerError> {
        self.multi_get_transaction_block_response_in_blocking_task_impl(&digests, options)
            .await
    }

    /// Returns `true` when all basic data for the transaction (objects,
    /// displays, etc.) has been persisted by either the checkpoint or
    /// optimistic path.
    ///
    /// - Optimistic transactions: `optimistic_sequence_number > 0` (objects are
    ///   committed atomically with the tx)
    /// - Checkpoint transactions: the latest indexed checkpoint's
    ///   `max_tx_sequence_number >= chk_tx_sequence_number`, meaning the
    ///   checkpoint containing this tx has been fully persisted
    pub(crate) async fn is_transaction_fully_indexed(
        &self,
        digest: TransactionDigest,
    ) -> IndexerResult<bool> {
        self.spawn_blocking(move |this| {
            let digest_bytes = digest.inner().to_vec();
            let global_order_entry = run_query!(&this.pool, |conn| {
                tx_global_order::table
                    .filter(tx_global_order::tx_digest.eq(digest_bytes))
                    .select((
                        tx_global_order::optimistic_sequence_number,
                        tx_global_order::chk_tx_sequence_number,
                    ))
                    .first::<(i64, Option<i64>)>(conn)
                    .optional()
            })?;

            match global_order_entry {
                // Optimistic tx: objects committed atomically.
                Some((opt_seq, _)) if opt_seq > 0 => Ok(true),
                // Checkpoint tx: check if the latest indexed checkpoint covers this tx.
                Some((_, Some(tx_seq))) => {
                    let max_indexed_tx = run_query!(&this.pool, |conn| {
                        checkpoints::table
                            .order(checkpoints::sequence_number.desc())
                            .select(checkpoints::max_tx_sequence_number)
                            .first::<Option<i64>>(conn)
                            .optional()
                    })?;
                    Ok(max_indexed_tx
                        .flatten()
                        .is_some_and(|max_tx| max_tx >= tx_seq))
                }
                // Row not found or chk_tx_sequence_number not yet set.
                _ => Ok(false),
            }
        })
        .await
    }

    pub async fn multi_get_transaction_block_response_in_blocking_task_with_preserved_order(
        &self,
        ordered_digests: Vec<TransactionDigest>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> Result<Vec<IotaTransactionBlockResponse>, IndexerError> {
        let order_map: HashMap<TransactionDigest, usize> = ordered_digests
            .iter()
            .enumerate()
            .map(|(index, &id)| (id, index))
            .collect();

        let mut transactions = self
            .multi_get_transaction_block_response_in_blocking_task_impl(&ordered_digests, options)
            .await?;
        transactions.sort_unstable_by_key(|tx| {
            order_map
                .get(&tx.digest)
                .copied()
                .expect("all digests should have some order")
        });
        Ok(transactions)
    }

    /// Fetches transaction events from the indexer storage.
    ///
    /// Retrieval order:
    /// 1. Checkpointed data (finalized transactions)
    /// 2. Optimistic data (pending transactions not yet checkpointed)
    /// 3. Historical fallback storage (if enabled)
    ///
    /// Returns [`IndexerError::DataPruned`] if the data is not available
    /// in Postgres database.
    pub async fn get_transaction_events_with_fallback(
        &self,
        digest: TransactionDigest,
    ) -> Result<Vec<iota_json_rpc_types::IotaEvent>, IndexerError> {
        if let Some((timestamp_ms, serialized_events)) = self
            .db()
            .try_get_checkpointed_transaction_events(digest)
            .await?
        {
            return self
                .convert_stored_events(digest, serialized_events, Some(timestamp_ms as u64))
                .await;
        }

        if let Some(serialized_events) = self.db().get_optimistic_transaction_events(digest).await?
        {
            return self
                .convert_stored_events(digest, serialized_events, None)
                .await;
        }

        if let Some(fallback) = self.fallback_reader() {
            return fallback.all_events(digest).await;
        }

        Err(IndexerError::DataPruned(
            "requested events not available".into(),
        ))
    }

    /// Converts [`StoredTransactionEvents`] into
    /// [`IotaEvent`](iota_json_rpc_types::IotaEvent).
    async fn convert_stored_events(
        &self,
        digest: TransactionDigest,
        serialized_events: StoredTransactionEvents,
        timestamp_ms: Option<u64>,
    ) -> Result<Vec<iota_json_rpc_types::IotaEvent>, IndexerError> {
        let events = stored_events_to_events(serialized_events)?;
        let tx_events = TransactionEvents { data: events };
        tx_events_to_iota_tx_events(tx_events, self.package_resolver(), digest, timestamp_ms)
            .await
            .map(|iota_tx_event| iota_tx_event.data)
    }

    async fn query_events_by_tx_digest_with_fallback(
        &self,
        tx_digest: TransactionDigest,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IotaEvent>> {
        let db_res = self
            .db()
            .query_events_by_tx_digest(tx_digest, cursor, limit, descending_order)
            .await;

        if let (Err(IndexerError::DataPruned(err)), Some(kv_reader)) =
            (db_res.as_ref(), self.fallback_reader())
        {
            kv_reader
                .events(tx_digest, cursor, limit, descending_order)
                .await
                .context(&format!("fallback triggered by {err}"))
        } else {
            let mut iota_event_futures = vec![];
            for stored_event in db_res? {
                iota_event_futures.push(tokio::task::spawn(
                    stored_event.try_into_iota_event(self.package_resolver.clone()),
                ));
            }

            futures::future::join_all(iota_event_futures)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| tracing::error!("failed to join iota event futures: {e}"))?
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| tracing::error!("failed to collect iota event futures: {e}"))
        }
    }

    pub(crate) async fn query_only_checkpointed_events_in_blocking_task(
        &self,
        filter: EventFilter,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IotaEvent>> {
        if let EventFilter::Transaction(tx_digest) = filter {
            return self
                .query_events_by_tx_digest_with_fallback(tx_digest, cursor, limit, descending_order)
                .await;
        }

        // All event-related tables that could be used by any filter
        let event_tables = [
            CommitterTables::Events,
            CommitterTables::EventEmitPackage,
            CommitterTables::EventEmitModule,
            CommitterTables::EventSenders,
            CommitterTables::EventStructPackage,
            CommitterTables::EventStructModule,
            CommitterTables::EventStructName,
            CommitterTables::EventStructInstantiation,
            CommitterTables::TxSenders,
        ];
        let min_available_tx = self
            .watermark_cache
            .get_lowest_available_tx_for_tables(&event_tables)
            .unwrap_or(0);

        let (tx_seq, event_seq) = if let Some(cursor) = cursor {
            let EventID {
                tx_digest,
                event_seq,
            } = cursor;
            let tx_seq: i64 = self
                .db()
                .resolve_cursor_tx_digest_to_seq_num(tx_digest)
                .await?;
            self.ensure_data_not_pruned_for_tx(tx_seq, &event_tables)?;
            (tx_seq, event_seq as i64)
        } else if descending_order {
            let max_tx_seq = i64::MAX;
            let max_event_seq = i64::MAX;
            (max_tx_seq, max_event_seq)
        } else {
            (-1, 0)
        };

        let query = if let EventFilter::Sender(sender) = &filter {
            // Need to remove ambiguities for tx_sequence_number column
            let cursor_clause = if descending_order {
                format!(
                    "(e.{TX_SEQUENCE_NUMBER_STR} < {tx_seq} OR (e.{TX_SEQUENCE_NUMBER_STR} = {tx_seq} AND e.{EVENT_SEQUENCE_NUMBER_STR} < {event_seq}))"
                )
            } else {
                format!(
                    "(e.{TX_SEQUENCE_NUMBER_STR} > {tx_seq} OR (e.{TX_SEQUENCE_NUMBER_STR} = {tx_seq} AND e.{EVENT_SEQUENCE_NUMBER_STR} > {event_seq}))"
                )
            };
            let order_clause = if descending_order {
                format!("e.{TX_SEQUENCE_NUMBER_STR} DESC, e.{EVENT_SEQUENCE_NUMBER_STR} DESC")
            } else {
                format!("e.{TX_SEQUENCE_NUMBER_STR} ASC, e.{EVENT_SEQUENCE_NUMBER_STR} ASC")
            };
            format!(
                "( \
                    SELECT *
                    FROM tx_senders s
                    JOIN events e
                    ON e.tx_sequence_number = s.tx_sequence_number
                    AND s.sender = '\\x{}'::bytea
                    WHERE e.tx_sequence_number >= {} AND ({}) \
                    ORDER BY {} \
                    LIMIT {}
                )",
                Hex::encode(sender.as_bytes()),
                min_available_tx,
                cursor_clause,
                order_clause,
                limit,
            )
        } else if let EventFilter::Transaction(_) = filter {
            unreachable!("case handled earlier in the function")
        } else {
            let main_where_clause = match filter {
                EventFilter::Package(package_id) => {
                    format!("package = '\\x{}'::bytea", package_id.to_raw_hex())
                }
                EventFilter::MoveModule { package, module } => {
                    format!(
                        "package = '\\x{}'::bytea AND module = '{}'",
                        package.to_raw_hex(),
                        module,
                    )
                }
                EventFilter::MoveEventType(struct_tag) => {
                    let formatted_struct_tag = struct_tag.to_canonical_string(true);
                    format!("event_type = '{formatted_struct_tag}'")
                }
                EventFilter::MoveEventModule { package, module } => {
                    let package_module_prefix = format!("{}::{}", package.to_short_hex(), module);
                    format!("event_type LIKE '{package_module_prefix}::%'")
                }
                EventFilter::Sender(_) => {
                    // Processed above
                    unreachable!()
                }
                EventFilter::Transaction(_) => {
                    // Processed above
                    unreachable!()
                }
                EventFilter::MoveEventField { .. }
                | EventFilter::All(_)
                | EventFilter::Any(_)
                | EventFilter::And(_, _)
                | EventFilter::Or(_, _)
                | EventFilter::TimeRange { .. } => {
                    return Err(IndexerError::NotSupported(
                        "This type of EventFilter is not supported.".into(),
                    ));
                }
            };

            let cursor_clause = if descending_order {
                format!(
                    "AND ({TX_SEQUENCE_NUMBER_STR} < {tx_seq} OR ({TX_SEQUENCE_NUMBER_STR} = {tx_seq} AND {EVENT_SEQUENCE_NUMBER_STR} < {event_seq}))"
                )
            } else {
                format!(
                    "AND ({TX_SEQUENCE_NUMBER_STR} > {tx_seq} OR ({TX_SEQUENCE_NUMBER_STR} = {tx_seq} AND {EVENT_SEQUENCE_NUMBER_STR} > {event_seq}))"
                )
            };
            let order_clause = if descending_order {
                format!("{TX_SEQUENCE_NUMBER_STR} DESC, {EVENT_SEQUENCE_NUMBER_STR} DESC")
            } else {
                format!("{TX_SEQUENCE_NUMBER_STR} ASC, {EVENT_SEQUENCE_NUMBER_STR} ASC")
            };

            format!(
                "
                    SELECT * FROM events \
                    WHERE ({main_where_clause}) AND {TX_SEQUENCE_NUMBER_STR} >= {min_available_tx} {cursor_clause} \
                    ORDER BY {order_clause} \
                    LIMIT {limit}
                ",
            )
        };
        tracing::debug!("query events: {}", query);
        let pool = self.get_pool();
        let stored_events = run_query_async!(&pool, move |conn| diesel::sql_query(query)
            .load::<StoredEvent>(conn))?;
        let mut iota_event_futures = vec![];
        for stored_event in stored_events {
            iota_event_futures.push(tokio::task::spawn(
                stored_event.try_into_iota_event(self.package_resolver.clone()),
            ));
        }
        let iota_events = futures::future::join_all(iota_event_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("failed to join iota event futures: {e}"))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("failed to collect iota event futures: {e}"))?;
        Ok(iota_events)
    }

    pub async fn get_dynamic_fields_in_blocking_task(
        &self,
        parent_object_id: ObjectID,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> Result<Vec<DynamicFieldInfo>, IndexerError> {
        let stored_objects = self
            .spawn_blocking(move |this| {
                this.get_dynamic_fields_raw(parent_object_id, cursor, limit)
            })
            .await?;

        let mut df_futures = vec![];
        let read_arc = Arc::new(self.clone());
        for stored_object in stored_objects {
            let read_arc_clone = Arc::clone(&read_arc);
            df_futures.push(tokio::task::spawn(async move {
                read_arc_clone
                    .try_create_dynamic_field_info(stored_object)
                    .await
            }));
        }
        let df_infos = futures::future::try_join_all(df_futures)
            .await
            .tap_err(|e| tracing::error!("error joining DF futures: {e:?}"))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| {
                tracing::error!("error calling DF try_create_dynamic_field_info function: {e:?}")
            })?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        Ok(df_infos)
    }

    pub async fn get_dynamic_fields_raw_in_blocking_task(
        &self,
        parent_object_id: ObjectID,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        self.spawn_blocking(move |this| {
            this.get_dynamic_fields_raw(parent_object_id, cursor, limit)
        })
        .await
    }

    fn get_dynamic_fields_raw(
        &self,
        parent_object_id: ObjectID,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, IndexerError> {
        let objects: Vec<StoredObject> = run_query!(&self.pool, |conn| {
            let mut query = objects::dsl::objects
                .filter(objects::dsl::owner_type.eq(OwnerType::Object as i16))
                .filter(objects::dsl::owner_id.eq(parent_object_id.as_bytes()))
                .order(objects::dsl::object_id.asc())
                .limit(limit as i64)
                .into_boxed();
            if let Some(object_cursor) = cursor {
                query = query.filter(objects::dsl::object_id.gt(object_cursor.as_bytes().to_vec()));
            }
            query.load::<StoredObject>(conn)
        })?;

        Ok(objects)
    }

    async fn try_create_dynamic_field_info(
        &self,
        stored_object: StoredObject,
    ) -> Result<Option<DynamicFieldInfo>, IndexerError> {
        if stored_object.df_kind.is_none() {
            return Ok(None);
        }

        let object: Object = stored_object.try_into()?;
        let Some(move_object) = object.data.as_struct_opt().cloned() else {
            return Err(IndexerError::ResolveMoveStruct(
                "Object is not a MoveObject".to_string(),
            ));
        };
        let type_tag = move_object.type_tag();
        let layout = self
            .package_resolver
            .type_layout(type_tag.clone())
            .await
            .map_err(|e| {
                IndexerError::ResolveMoveStruct(format!(
                    "Failed to get type layout for type {type_tag}: {e}",
                ))
            })?;

        let field = DFV::FieldVisitor::deserialize(move_object.contents(), &layout)
            .tap_err(|e| tracing::warn!("{e}"))?;

        let type_ = field.kind;
        let name_type: TypeTag = type_tag_core_to_sdk(&field.name_layout.into());
        let bcs_name = field.name_bytes.to_owned();

        let name_value = BoundedVisitor::deserialize_value(field.name_bytes, field.name_layout)
            .tap_err(|e| tracing::warn!("{e}"))?;

        let name = DynamicFieldName {
            type_: name_type,
            value: IotaMoveValue::from(name_value).to_json_value(),
        };

        let value_metadata = field.value_metadata().map_err(|e| {
            tracing::warn!("{e}");
            IndexerError::Uncategorized(anyhow!(e))
        })?;

        Ok(Some(match value_metadata {
            DFV::ValueMetadata::DynamicField(object_type) => DynamicFieldInfo {
                name,
                bcs_name,
                type_,
                object_type: object_type.to_canonical_string(/* with_prefix */ true),
                object_id: object.id(),
                version: object.version(),
                digest: object.digest(),
            },

            DFV::ValueMetadata::DynamicObjectField(object_id) => {
                let object = self
                    .get_object_in_blocking_task(object_id)
                    .await?
                    .ok_or_else(|| {
                        IndexerError::Uncategorized(anyhow!(
                            "Failed to find object_id {} when trying to create dynamic field info",
                            object_id.to_canonical_string(/* with_prefix */ true),
                        ))
                    })?;

                let object_type = object.data.object_type().unwrap().clone();
                DynamicFieldInfo {
                    name,
                    bcs_name,
                    type_,
                    object_type: object_type.to_canonical_string(/* with_prefix */ true),
                    object_id,
                    version: object.version(),
                    digest: object.digest(),
                }
            }
        }))
    }

    pub async fn bcs_name_from_dynamic_field_name(
        &self,
        name: &DynamicFieldName,
    ) -> Result<Vec<u8>, IndexerError> {
        let move_type_layout = self
            .package_resolver()
            .type_layout(name.type_.clone())
            .await
            .map_err(|e| {
                IndexerError::ResolveMoveStruct(format!(
                    "Failed to get type layout for type {}: {}",
                    name.type_, e
                ))
            })?;
        let iota_json_value = iota_json::IotaJsonValue::new(name.value.clone())?;
        let name_bcs_value = iota_json_value.to_bcs_bytes(&move_type_layout)?;
        Ok(name_bcs_value)
    }

    pub async fn get_display_object_by_type(
        &self,
        object_type: &StructTag,
    ) -> Result<Option<iota_types::display::DisplayVersionUpdatedEvent>, IndexerError> {
        let object_type = object_type.to_canonical_string(/* with_prefix */ true);
        self.spawn_blocking(move |this| this.get_display_update_event(object_type))
            .await
    }

    fn get_display_update_event(
        &self,
        object_type: String,
    ) -> Result<Option<iota_types::display::DisplayVersionUpdatedEvent>, IndexerError> {
        let stored_display = run_query!(&self.pool, |conn| {
            display::table
                .filter(display::object_type.eq(object_type))
                .first::<StoredDisplay>(conn)
                .optional()
        })?;

        let stored_display = match stored_display {
            Some(display) => display,
            None => return Ok(None),
        };

        let display_update = stored_display.to_display_update_event()?;

        Ok(Some(display_update))
    }

    pub async fn get_owned_coins_in_blocking_task(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
        cursor: ObjectID,
        limit: usize,
    ) -> Result<Vec<IotaCoin>, IndexerError> {
        self.spawn_blocking(move |this| this.get_owned_coins(owner, coin_type, cursor, limit))
            .await
    }

    fn get_owned_coins(
        &self,
        owner: IotaAddress,
        // If coin_type is None, look for all coins.
        coin_type: Option<String>,
        cursor: ObjectID,
        limit: usize,
    ) -> Result<Vec<IotaCoin>, IndexerError> {
        let mut query = objects::dsl::objects
            .filter(objects::dsl::owner_type.eq(OwnerType::Address as i16))
            .filter(objects::dsl::owner_id.eq(owner.as_bytes()))
            .filter(objects::dsl::object_id.gt(cursor.as_bytes()))
            .into_boxed();
        if let Some(coin_type) = coin_type {
            query = query.filter(objects::dsl::coin_type.eq(Some(coin_type)));
        } else {
            query = query.filter(objects::dsl::coin_type.is_not_null());
        }
        query = query
            .order(objects::dsl::object_id.asc())
            .limit(limit as i64);

        let stored_objects = run_query!(&self.pool, |conn| query.load::<StoredObject>(conn))?;

        stored_objects
            .into_iter()
            .map(|o| o.try_into())
            .collect::<IndexerResult<Vec<_>>>()
    }

    pub async fn get_coin_balances_in_blocking_task(
        &self,
        owner: IotaAddress,
        // If coin_type is None, look for all coins.
        coin_type: Option<String>,
    ) -> Result<Vec<Balance>, IndexerError> {
        self.spawn_blocking(move |this| this.get_coin_balances(owner, coin_type))
            .await
    }

    fn get_coin_balances(
        &self,
        owner: IotaAddress,
        // If coin_type is None, look for all coins.
        coin_type: Option<String>,
    ) -> Result<Vec<Balance>, IndexerError> {
        let coin_type_filter = if let Some(coin_type) = coin_type {
            format!("= '{coin_type}'")
        } else {
            "IS NOT NULL".to_string()
        };
        // Note: important to cast to BIGINT to avoid deserialize confusion
        let query = format!(
            "
            SELECT coin_type, \
            CAST(COUNT(*) AS BIGINT) AS coin_num, \
            CAST(SUM(coin_balance) AS BIGINT) AS coin_balance \
            FROM objects \
            WHERE owner_type = {} \
            AND owner_id = '\\x{}'::BYTEA \
            AND coin_type {} \
            GROUP BY coin_type \
            ORDER BY coin_type ASC
        ",
            OwnerType::Address as i16,
            Hex::encode(owner.as_bytes()),
            coin_type_filter,
        );

        tracing::debug!("get coin balances query: {query}");
        let coin_balances = run_query!(&self.pool, |conn| diesel::sql_query(query)
            .load::<CoinBalance>(conn))?;
        coin_balances
            .into_iter()
            .map(|cb| cb.try_into())
            .collect::<IndexerResult<Vec<_>>>()
    }

    pub fn get_latest_network_metrics(&self) -> IndexerResult<NetworkMetrics> {
        let mut metrics = run_query!(&self.pool, |conn| {
            diesel::sql_query("SELECT * FROM network_metrics;")
                .get_result::<StoredNetworkMetrics>(conn)
        })?;
        if metrics.total_addresses == -1 {
            // this implies that the estimate is not available in the db
            // so we fallback to the more expensive count query
            metrics.total_addresses = run_query!(&self.pool, |conn| {
                addresses::dsl::addresses.count().get_result::<i64>(conn)
            })?;
        }
        if metrics.total_packages == -1 {
            // this implies that the estimate is not available in the db
            // so we fallback to the more expensive count query
            metrics.total_packages = run_query!(&self.pool, |conn| {
                packages::dsl::packages.count().get_result::<i64>(conn)
            })?;
        }
        Ok(metrics.into())
    }

    /// Get the latest move call metrics.
    pub fn get_latest_move_call_metrics(&self) -> IndexerResult<MoveCallMetrics> {
        let latest_3_days = self.get_latest_move_call_metrics_by_day(3)?;
        let latest_7_days = self.get_latest_move_call_metrics_by_day(7)?;
        let latest_30_days = self.get_latest_move_call_metrics_by_day(30)?;

        // sort by call count desc.
        let rank_3_days = latest_3_days
            .into_iter()
            .sorted_by(|a, b| b.1.cmp(&a.1))
            .collect::<Vec<_>>();
        let rank_7_days = latest_7_days
            .into_iter()
            .sorted_by(|a, b| b.1.cmp(&a.1))
            .collect::<Vec<_>>();
        let rank_30_days = latest_30_days
            .into_iter()
            .sorted_by(|a, b| b.1.cmp(&a.1))
            .collect::<Vec<_>>();

        Ok(MoveCallMetrics {
            rank_3_days,
            rank_7_days,
            rank_30_days,
        })
    }

    /// Get the latest move call metrics by day.
    pub fn get_latest_move_call_metrics_by_day(
        &self,
        day_value: i64,
    ) -> IndexerResult<Vec<(MoveFunctionName, usize)>> {
        let query = "
            SELECT id, epoch, day, move_package, move_module, move_function, count
            FROM move_call_metrics
            WHERE day = $1
              AND epoch = (SELECT MAX(epoch) FROM move_call_metrics WHERE day = $1)
            ORDER BY count DESC
            LIMIT 10
        ";

        let queried_metrics = run_query!(&self.pool, |conn| sql_query(query)
            .bind::<BigInt, _>(day_value)
            .load::<QueriedMoveCallMetrics>(conn))?;

        let metrics = queried_metrics
            .into_iter()
            .map(|m| {
                m.try_into()
                    .map_err(|e| diesel::result::Error::DeserializationError(Box::new(e)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(metrics)
    }

    pub fn get_latest_address_metrics(&self) -> IndexerResult<AddressMetrics> {
        let stored_address_metrics = run_query!(&self.pool, |conn| {
            address_metrics::table
                .order(address_metrics::dsl::checkpoint.desc())
                .first::<StoredAddressMetrics>(conn)
        })?;
        Ok(stored_address_metrics.into())
    }

    pub fn get_checkpoint_address_metrics(
        &self,
        checkpoint_seq: u64,
    ) -> IndexerResult<AddressMetrics> {
        let stored_address_metrics = run_query!(&self.pool, |conn| {
            address_metrics::table
                .filter(address_metrics::dsl::checkpoint.eq(checkpoint_seq as i64))
                .first::<StoredAddressMetrics>(conn)
        })?;
        Ok(stored_address_metrics.into())
    }

    pub fn get_all_epoch_address_metrics(
        &self,
        descending_order: Option<bool>,
    ) -> IndexerResult<Vec<AddressMetrics>> {
        let is_descending = descending_order.unwrap_or_default();
        let epoch_address_metrics_query = format!(
            "WITH ranked_rows AS (
                SELECT
                  checkpoint, epoch, timestamp_ms, cumulative_addresses, cumulative_active_addresses, daily_active_addresses,
                  row_number() OVER(PARTITION BY epoch ORDER BY checkpoint DESC) as row_num
                FROM
                  address_metrics
              )
              SELECT
                checkpoint, epoch, timestamp_ms, cumulative_addresses, cumulative_active_addresses, daily_active_addresses
              FROM ranked_rows
              WHERE row_num = 1 ORDER BY epoch {}",
            if is_descending { "DESC" } else { "ASC" },
        );
        let epoch_address_metrics = run_query!(&self.pool, |conn| {
            diesel::sql_query(epoch_address_metrics_query).load::<StoredAddressMetrics>(conn)
        })?;

        Ok(epoch_address_metrics
            .into_iter()
            .map(|stored_address_metrics| stored_address_metrics.into())
            .collect())
    }

    pub(crate) async fn get_display_fields(
        &self,
        original_object: &iota_types::object::Object,
        original_layout: &Option<MoveStructLayout>,
    ) -> Result<DisplayFieldsResponse, IndexerError> {
        let (object_type, layout) = if let Some((object_type, layout)) =
            iota_json_rpc::read_api::get_object_type_and_struct(original_object, original_layout)
                .map_err(|e| IndexerError::Generic(e.to_string()))?
        {
            (object_type, layout)
        } else {
            return Ok(DisplayFieldsResponse {
                data: None,
                error: None,
            });
        };

        if let Some(display_object) = self.get_display_object_by_type(&object_type).await? {
            return iota_json_rpc::read_api::get_rendered_fields(display_object.fields, &layout)
                .map_err(|e| IndexerError::Generic(e.to_string()));
        }
        Ok(DisplayFieldsResponse {
            data: None,
            error: None,
        })
    }

    pub async fn get_coin_metadata_in_blocking_task(
        &self,
        coin_struct: StructTag,
    ) -> Result<Option<IotaCoinMetadata>, IndexerError> {
        self.spawn_blocking(move |this| this.get_coin_metadata(coin_struct))
            .await
    }

    fn get_coin_metadata(
        &self,
        coin_struct: StructTag,
    ) -> Result<Option<IotaCoinMetadata>, IndexerError> {
        let coin_metadata_type = StructTag::new_coin_metadata(coin_struct.clone());
        let metadata_object = self
            .get_singleton_object(coin_metadata_type)?
            .and_then(|o| IotaCoinMetadata::try_from(o).ok());

        if let Some(metadata_object) = metadata_object {
            Ok(Some(metadata_object))
        } else {
            let coin_manager_obj = self.get_coin_manager_obj(coin_struct)?;
            Ok(
                coin_manager_obj.and_then(|m| match (m.metadata, m.immutable_metadata) {
                    (Some(metadata), _) => Some(metadata.into()),
                    (_, Some(immutable_metadata)) => Some(IotaCoinMetadata {
                        decimals: immutable_metadata.decimals,
                        name: immutable_metadata.name,
                        symbol: immutable_metadata.symbol,
                        description: immutable_metadata.description,
                        icon_url: immutable_metadata.icon_url,
                        id: None,
                    }),
                    (None, None) => None,
                }),
            )
        }
    }

    fn get_coin_manager_obj(
        &self,
        coin_type: StructTag,
    ) -> Result<Option<CoinManager>, IndexerError> {
        let coin_manager_type = StructTag::new_coin_manager(coin_type);
        let coin_manager_object = self
            .get_singleton_object(coin_manager_type)?
            .and_then(|o| CoinManager::try_from(o).ok());
        Ok(coin_manager_object)
    }

    pub async fn get_total_supply_in_blocking_task(
        &self,
        coin_struct: StructTag,
    ) -> Result<Supply, IndexerError> {
        self.spawn_blocking(move |this| this.get_total_supply(coin_struct))
            .await
    }

    fn get_total_supply(&self, coin_struct: StructTag) -> Result<Supply, IndexerError> {
        if let Some(supply) = self.get_treasury_cap_total_supply(&coin_struct)? {
            return Ok(supply);
        }
        if let Some(supply) = self.get_coin_manager_total_supply(&coin_struct)? {
            return Ok(supply);
        }
        Err(IndexerError::Generic(format!(
            "Cannot find treasury cap or coin manager for coin type: {}",
            coin_struct.to_canonical_string(/* with_prefix */ true)
        )))
    }

    fn get_treasury_cap_total_supply(
        &self,
        coin_struct: &StructTag,
    ) -> Result<Option<Supply>, IndexerError> {
        let tag = StructTag::new_treasury_cap(coin_struct.clone());
        Ok(self
            .get_object_as::<TreasuryCap>(tag)?
            .map(|tc| tc.total_supply))
    }

    fn get_coin_manager_total_supply(
        &self,
        coin_struct: &StructTag,
    ) -> Result<Option<Supply>, IndexerError> {
        let tag = StructTag::new_coin_manager(coin_struct.clone());
        Ok(self
            .get_object_as::<CoinManager>(tag)?
            .map(|mgr| mgr.treasury_cap.total_supply))
    }

    fn get_object_as<T>(&self, tag: StructTag) -> Result<Option<T>, IndexerError>
    where
        T: TryFrom<Object, Error = IotaError>,
    {
        let cache_key = tag.to_canonical_string(/* with_prefix */ true);

        let mut cache = self
            .obj_type_cache
            .lock()
            .inspect_err(|e| tracing::error!("cache poisoned: {e:?}"))
            .map_err(|_| IndexerError::Generic("failed to lock cache".into()))?;

        let maybe_obj = match cache.cache_get(&cache_key) {
            Some(Some(id)) => self.get_object(id, None).ok().flatten(),
            _ => {
                let fetched = self.get_singleton_object(tag)?;
                cache.cache_set(cache_key.clone(), fetched.as_ref().map(|o| o.id()));
                fetched
            }
        };

        Ok(maybe_obj.map(T::try_from).transpose()?)
    }

    pub fn get_consistent_read_range(&self) -> Result<(i64, i64), IndexerError> {
        let latest_checkpoint_sequence = run_query!(&self.pool, |conn| {
            checkpoints::table
                .select(checkpoints::sequence_number)
                .order(checkpoints::sequence_number.desc())
                .first::<i64>(conn)
                .optional()
        })?
        .unwrap_or_default();
        let latest_object_snapshot_checkpoint_sequence = run_query!(&self.pool, |conn| {
            objects_snapshot::table
                .select(objects_snapshot::checkpoint_sequence_number)
                .order(objects_snapshot::checkpoint_sequence_number.desc())
                .first::<i64>(conn)
                .optional()
        })?
        .unwrap_or_default();
        Ok((
            latest_object_snapshot_checkpoint_sequence,
            latest_checkpoint_sequence,
        ))
    }

    pub fn package_resolver(&self) -> &PackageResolver {
        &self.package_resolver
    }

    pub async fn pending_active_validators(
        &self,
    ) -> Result<Vec<IotaValidatorSummary>, IndexerError> {
        self.spawn_blocking(move |this| {
            iota_types::iota_system_state::get_iota_system_state(&this)
                .and_then(|system_state| system_state.get_pending_active_validators(&this))
        })
        .await
        .map_err(Into::into)
    }

    /// Get the participation metrics. Participation is defined as the total
    /// number of unique addresses that have delegated stake in the current
    /// epoch. Includes both staked and timelocked staked IOTA.
    pub fn get_participation_metrics(&self) -> IndexerResult<ParticipationMetrics> {
        run_query!(&self.pool, |conn| {
            diesel::sql_query("SELECT * FROM participation_metrics")
                .get_result::<StoredParticipationMetrics>(conn)
        })
        .map(Into::into)
    }
}

impl iota_types::storage::ObjectStore for IndexerReader {
    fn try_get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<iota_types::object::Object>, iota_types::storage::error::Error> {
        self.get_object(object_id, None)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: iota_types::base_types::VersionNumber,
    ) -> Result<Option<iota_types::object::Object>, iota_types::storage::error::Error> {
        self.get_object(object_id, Some(version))
            .map_err(iota_types::storage::error::Error::custom)
    }
}

#[async_trait]
impl DataReader for IndexerReader {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        object_type: StructTag,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
        options: IotaObjectDataOptions,
    ) -> Result<iota_json_rpc_types::ObjectsPage, anyhow::Error> {
        let limit = limit.unwrap_or(50);
        let mut stored_objects = self
            .get_owned_objects_in_blocking_task(
                address,
                Some(IotaObjectDataFilter::StructType(object_type)),
                cursor,
                limit + 1,
            )
            .await?;

        let mut res = Vec::new();

        let mut next_cursor = None;
        if stored_objects.len() > limit && limit > 0 {
            // Here the cursor is the last object id in the previous page
            stored_objects.pop().unwrap();
            next_cursor = Some(if let Some(last_object) = stored_objects.last() {
                last_object.get_object_ref()?.object_id
            } else {
                ObjectID::ZERO
            });
        }

        for stored_object in stored_objects {
            let read = stored_object
                .try_into_object_read(&self.package_resolver)
                .await?;
            res.push(IotaObjectResponse::try_from_object_read_and_options(
                read, &options,
            )?);
        }

        Ok(iota_json_rpc_types::ObjectsPage {
            data: res,
            next_cursor,
            has_next_page: next_cursor.is_some(),
        })
    }

    async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error> {
        let result = self.get_object_read_in_blocking_task(object_id).await?;
        Ok(IotaObjectResponse::try_from_object_read_and_options(
            result, &options,
        )?)
    }

    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        let epoch_info = GovernanceReadApi::new(self.clone())
            .get_epoch_info(None)
            .await?;
        Ok(epoch_info
            .reference_gas_price
            .ok_or_else(|| anyhow::anyhow!("missing latest reference_gas_price"))?)
    }
}

impl<'a> DBReader<'a> {
    pub fn new(reader: &'a IndexerReader) -> Self {
        Self {
            main_reader: reader,
        }
    }

    async fn query_transactions_by_checkpoint_seq(
        &self,
        checkpoint_seq: u64,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<StoredTransaction>> {
        self.main_reader.ensure_data_not_pruned_for_checkpoint(
            checkpoint_seq,
            &[
                CommitterTables::Transactions,
                CommitterTables::PrunerCpWatermark,
            ],
        )?;

        // After watermark checks, we can safely assume data is present in all tables
        let pool = self.main_reader.get_pool();
        let tx_range = run_query_async!(&pool, move |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select((
                    pruner_cp_watermark::min_tx_sequence_number,
                    pruner_cp_watermark::max_tx_sequence_number,
                ))
                // we filter the pruner_cp_watermark table because it is indexed by
                // checkpoint_sequence_number, transactions is not
                .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(checkpoint_seq as i64))
                .first::<(i64, i64)>(conn)
        })
        .context("failed to get transaction range from pruner_cp_watermark table")?;

        let cursor_tx_seq = if let Some(cursor) = cursor {
            Some(self.resolve_cursor_tx_digest_to_seq_num(cursor).await?)
        } else {
            None
        };

        let mut query = transactions::dsl::transactions
            .filter(transactions::tx_sequence_number.between(tx_range.0, tx_range.1))
            .into_boxed();

        // Translate transaction digest cursor to tx sequence number
        if let Some(cursor_tx_seq) = cursor_tx_seq {
            if is_descending {
                query = query.filter(transactions::dsl::tx_sequence_number.lt(cursor_tx_seq));
            } else {
                query = query.filter(transactions::dsl::tx_sequence_number.gt(cursor_tx_seq));
            }
        }
        if is_descending {
            query = query.order(transactions::dsl::tx_sequence_number.desc());
        } else {
            query = query.order(transactions::dsl::tx_sequence_number.asc());
        }
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, move |conn| query
            .limit(limit as i64)
            .load::<StoredTransaction>(conn))
    }

    async fn query_events_by_tx_digest(
        &self,
        tx_digest: TransactionDigest,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<StoredEvent>> {
        let mut query = events::table.into_boxed();

        if let Some(cursor) = cursor {
            if cursor.tx_digest != tx_digest {
                return Err(IndexerError::InvalidArgument(
                    "Cursor tx_digest does not match the tx_digest in the query.".into(),
                ));
            }
            if descending_order {
                query = query.filter(events::event_sequence_number.lt(cursor.event_seq as i64));
            } else {
                query = query.filter(events::event_sequence_number.gt(cursor.event_seq as i64));
            }
        } else if descending_order {
            query = query.filter(events::event_sequence_number.le(i64::MAX));
        } else {
            query = query.filter(events::event_sequence_number.ge(0));
        };

        if descending_order {
            query = query.order(events::event_sequence_number.desc());
        } else {
            query = query.order(events::event_sequence_number.asc());
        }

        query = query.filter(
            events::tx_sequence_number.nullable().eq(tx_digests::table
                .select(tx_digests::tx_sequence_number)
                // we filter the tx_digests table because it is indexed by digest,
                // events table is not
                .filter(tx_digests::tx_digest.eq(tx_digest.into_inner().to_vec()))
                .single_value()),
        );

        let pool = self.main_reader.get_pool();
        let query = query.limit(limit as i64);
        let db_events = run_query_async!(&pool, move |conn| { query.load::<StoredEvent>(conn) })?;
        if db_events.is_empty() && self.check_tx_pruned(tx_digest).await? {
            return Err(IndexerError::DataPruned(format!(
                "data for tx {tx_digest} potentially pruned"
            )));
        }

        Ok(db_events)
    }

    async fn check_tx_pruned(&self, tx_digest: TransactionDigest) -> IndexerResult<bool> {
        // there is no way to distinguish now between pruned, and not existing txs
        self.resolve_cursor_tx_digest_to_seq_num_maybe(tx_digest)
            .await
            .map(|seq| seq.is_none())
    }

    pub async fn resolve_cursor_tx_digest_to_seq_num(
        &self,
        cursor: TransactionDigest,
    ) -> IndexerResult<i64> {
        self.resolve_cursor_tx_digest_to_seq_num_maybe(cursor)
            .await?
            .ok_or_else(|| {
                IndexerError::PostgresRead(format!("transaction with digest {cursor} not found"))
            })
    }

    async fn resolve_cursor_tx_digest_to_seq_num_maybe(
        &self,
        cursor: TransactionDigest,
    ) -> IndexerResult<Option<i64>> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, move |conn| {
            tx_digests::table
                .select(tx_digests::tx_sequence_number)
                // we filter the tx_digests table because it is indexed by digest,
                // transactions (and other tables) are not
                .filter(tx_digests::tx_digest.eq(cursor.into_inner().to_vec()))
                .first::<i64>(conn)
                .optional()
        })
    }

    pub async fn try_get_checkpointed_transaction_events(
        &self,
        digest: TransactionDigest,
    ) -> IndexerResult<Option<(i64, StoredTransactionEvents)>> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, move |conn| {
            transactions::table
                .filter(
                    transactions::tx_sequence_number
                        .nullable()
                        .eq(tx_digests::table
                            .select(tx_digests::tx_sequence_number)
                            // we filter the tx_digests table because it is indexed by digest,
                            // transactions table is not
                            .filter(tx_digests::tx_digest.eq(digest.into_inner().to_vec()))
                            .single_value()),
                )
                .select((transactions::timestamp_ms, transactions::events))
                .first::<(i64, StoredTransactionEvents)>(conn)
                .optional()
        })
    }

    pub async fn get_optimistic_transaction_events(
        &self,
        digest: TransactionDigest,
    ) -> IndexerResult<Option<StoredTransactionEvents>> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, move |conn| {
            optimistic_transactions::table
                .inner_join(
                    tx_global_order::table.on(optimistic_transactions::global_sequence_number
                        .eq(tx_global_order::global_sequence_number)
                        .and(
                            optimistic_transactions::optimistic_sequence_number
                                .eq(tx_global_order::optimistic_sequence_number),
                        )),
                )
                // we filter the `tx_global_order` table because it is indexed by digest,
                // optimistic_transactions table is not
                .filter(tx_global_order::tx_digest.eq(digest.into_inner().to_vec()))
                .select(optimistic_transactions::events)
                .first::<StoredTransactionEvents>(conn)
                .optional()
        })
    }

    async fn get_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> IndexerResult<Option<StoredCheckpoint>> {
        // Check if checkpoint is pruned when querying by sequence number
        if let CheckpointId::SequenceNumber(seq) = checkpoint_id {
            self.main_reader
                .ensure_data_not_pruned_for_checkpoint(seq, &[CommitterTables::Checkpoints])?;
        }

        let pool = self.main_reader.get_pool();
        let checkpoint = run_query_async!(&pool, |conn| {
            match checkpoint_id {
                CheckpointId::SequenceNumber(seq) => checkpoints::dsl::checkpoints
                    .filter(checkpoints::sequence_number.eq(seq as i64))
                    .first::<StoredCheckpoint>(conn)
                    .optional(),
                CheckpointId::Digest(digest) => checkpoints::dsl::checkpoints
                    .filter(checkpoints::checkpoint_digest.eq(digest.into_inner().to_vec()))
                    .first::<StoredCheckpoint>(conn)
                    .optional(),
            }
        })?;

        // When querying by digest, check if the returned checkpoint is in the pruned
        // range
        if let CheckpointId::Digest(_) = checkpoint_id {
            if let Some(ref cp) = checkpoint {
                self.main_reader.ensure_data_not_pruned_for_checkpoint(
                    cp.sequence_number as u64,
                    &[CommitterTables::Checkpoints],
                )?;
            }
        }

        Ok(checkpoint)
    }

    async fn get_checkpoints(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<StoredCheckpoint>> {
        // Get min available checkpoint to filter out pruned data
        let min_available_cp = self
            .main_reader
            .watermark_cache
            .get_lowest_available_cp_for_tables(&[CommitterTables::Checkpoints])
            .unwrap_or(0);

        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, |conn| {
            let mut boxed_query = checkpoints::table.into_boxed();
            boxed_query = boxed_query.filter(checkpoints::sequence_number.ge(min_available_cp));

            if let Some(cursor) = cursor {
                if descending_order {
                    boxed_query =
                        boxed_query.filter(checkpoints::sequence_number.lt(cursor as i64));
                } else {
                    boxed_query =
                        boxed_query.filter(checkpoints::sequence_number.gt(cursor as i64));
                }
            }
            if descending_order {
                boxed_query = boxed_query.order_by(checkpoints::sequence_number.desc());
            } else {
                boxed_query = boxed_query.order_by(checkpoints::sequence_number.asc());
            }

            boxed_query
                .limit(limit as i64)
                .load::<StoredCheckpoint>(conn)
        })
    }

    async fn get_object_version(
        &self,
        object_id: ObjectID,
        object_version: SequenceNumber,
        before_version: bool,
    ) -> IndexerResult<Option<StoredObjectVersion>> {
        let object_version_num = object_version.as_u64() as i64;
        let pool = self.main_reader.get_pool();

        // query objects_version to find the requested version
        run_query_async!(&pool, move |conn| {
            let mut query = objects_version::dsl::objects_version
                .filter(objects_version::object_id.eq(object_id.as_bytes()))
                .into_boxed();

            if before_version {
                query = query.filter(objects_version::object_version.lt(object_version_num));
            } else {
                query = query.filter(objects_version::object_version.eq(object_version_num));
            }

            query
                .order_by(objects_version::object_version.desc())
                .limit(1)
                .first::<StoredObjectVersion>(conn)
                .optional()
        })
    }

    async fn latest_existing_object_version(
        &self,
        object_id: ObjectID,
    ) -> IndexerResult<Option<i64>> {
        let pool = self.main_reader.get_pool();

        run_query_async!(&pool, move |conn| {
            objects_version::dsl::objects_version
                .filter(objects_version::object_id.eq(object_id.as_bytes()))
                .order_by(objects_version::object_version.desc())
                .select(objects_version::object_version)
                .limit(1)
                .first::<i64>(conn)
                .optional()
        })
    }

    pub async fn get_stored_history_object(
        &self,
        object_id: ObjectID,
        object_version: i64,
        checkpoint_sequence_number: i64,
    ) -> IndexerResult<Option<StoredHistoryObject>> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, move |conn| {
            // Match on the primary key.
            let query = objects_history::dsl::objects_history
                .filter(objects_history::checkpoint_sequence_number.eq(checkpoint_sequence_number))
                .filter(objects_history::object_id.eq(object_id.as_bytes()))
                .filter(objects_history::object_version.eq(object_version))
                .into_boxed();

            query
                .order_by(objects_history::object_version.desc())
                .limit(1)
                .first::<StoredHistoryObject>(conn)
                .optional()
        })
    }

    async fn get_optimistic_transactions_with_cp_info(
        &self,
        digests: Vec<Vec<u8>>,
    ) -> IndexerResult<Vec<(OptimisticTransaction, Option<i64>)>> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, |conn| {
            optimistic_transactions::table
                .inner_join(
                    tx_global_order::table.on(optimistic_transactions::global_sequence_number
                        .eq(tx_global_order::global_sequence_number)
                        .and(
                            optimistic_transactions::optimistic_sequence_number
                                .eq(tx_global_order::optimistic_sequence_number),
                        )),
                )
                // we filter the `tx_global_order` table because it is indexed by digest,
                // optimistic_transactions table is not
                .filter(tx_global_order::tx_digest.eq_any(digests))
                .select((
                    OptimisticTransaction::as_select(),
                    tx_global_order::chk_tx_sequence_number,
                ))
                .load::<(OptimisticTransaction, Option<i64>)>(conn)
        })
    }

    async fn get_checkpointed_transactions(
        &self,
        digests: Vec<Vec<u8>>,
    ) -> IndexerResult<Vec<StoredTransaction>> {
        // Get min available transaction to filter out pruned data
        let min_available_tx = self
            .main_reader
            .watermark_cache
            .get_lowest_available_tx_for_tables(&[CommitterTables::Transactions])
            .unwrap_or(0);

        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, |conn| {
            // using two-step query to allow partition pruning during execution.
            let tx_sequence_numbers = tx_digests::table
                .filter(tx_digests::tx_digest.eq_any(&digests))
                .select(tx_digests::tx_sequence_number)
                .load::<i64>(conn)?;

            if tx_sequence_numbers.is_empty() {
                return Ok(vec![]);
            }

            transactions::table
                .filter(transactions::tx_sequence_number.eq_any(tx_sequence_numbers))
                // Filter out pruned transactions
                .filter(transactions::tx_sequence_number.ge(min_available_tx))
                .select(StoredTransaction::as_select())
                .load::<StoredTransaction>(conn)
        })
    }

    async fn get_optimistic_transactions(
        &self,
        digests: Vec<Vec<u8>>,
    ) -> Result<Vec<OptimisticTransaction>, IndexerError> {
        let pool = self.main_reader.get_pool();
        run_query_async!(&pool, |conn| {
            optimistic_transactions::table
                .inner_join(
                    tx_global_order::table.on(optimistic_transactions::global_sequence_number
                        .eq(tx_global_order::global_sequence_number)
                        .and(
                            optimistic_transactions::optimistic_sequence_number
                                .eq(tx_global_order::optimistic_sequence_number),
                        )),
                )
                // we filter the `tx_global_order` table because it is indexed by digest,
                // optimistic_transactions table is not
                .filter(tx_global_order::tx_digest.eq_any(digests))
                .select(OptimisticTransaction::as_select())
                .load::<OptimisticTransaction>(conn)
        })
    }

    async fn get_single_transaction(
        &self,
        digest: TransactionDigest,
    ) -> IndexerResult<Option<StoredTransaction>> {
        let digests = vec![digest.inner().to_vec()];
        let optimistic_tx_future = self
            .get_optimistic_transactions_with_cp_info(digests.clone())
            .map(|result| result.map(|mut txs| txs.pop()));
        let checkpointed_tx_future = self
            .get_checkpointed_transactions(digests.clone())
            .map(|result| result.map(|mut txs| txs.pop()));

        tokio::pin!(optimistic_tx_future, checkpointed_tx_future);

        let result = tokio::select! {
                checkpointed_tx = &mut checkpointed_tx_future => match checkpointed_tx? {
                    Some(checkpointed_tx) => Some(checkpointed_tx),
                    None => optimistic_tx_future
                        .await?
                        .map(|(optimistic_tx, _)| optimistic_tx.into()),
                },
                optimistic_tx_with_cp_info = &mut optimistic_tx_future => {
                    match optimistic_tx_with_cp_info? {
                        Some((optimistic_tx, Some(_cp_info))) => Some(
                            checkpointed_tx_future
                                .await?
                                .unwrap_or_else(|| optimistic_tx.into()),
                        ),
                        Some((optimistic_tx, None)) => Some(optimistic_tx.into()),
                        None => checkpointed_tx_future.await?,
                    }
                }
        };

        Ok(result)
    }

    /// Fetches the latest transaction sequence number from the checkpoints
    /// table.
    pub async fn latest_tx_sequence_number(&self) -> Result<Option<i64>, IndexerError> {
        use crate::schema::checkpoints::dsl;

        let pool = self.main_reader.get_pool();

        run_query_async!(&pool, |conn| {
            dsl::checkpoints
                .select(dsl::max_tx_sequence_number)
                .order(dsl::sequence_number.desc())
                .first::<Option<i64>>(conn)
                .optional()
        })
        .map(Option::flatten)
    }
}

enum TransactionFilterKind {
    V1(TransactionFilter),
    V2(TransactionFilterV2),
}
