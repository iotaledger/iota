// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use cached::{Cached, SizedCache};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, OptionalExtension, PgConnection,
    QueryDsl, RunQueryDsl, SelectableHelper, TextExpressionMethods,
    dsl::sql,
    r2d2::ConnectionManager,
    sql_query,
    sql_types::{BigInt, Bool},
};
use fastcrypto::encoding::{Encoding, Hex};
use iota_json_rpc_types::{
    AddressMetrics, Balance, CheckpointId, Coin as IotaCoin, DisplayFieldsResponse, EpochInfo,
    EventFilter, IotaCoinMetadata, IotaEvent, IotaMoveValue, IotaObjectDataFilter,
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionKind, MoveCallMetrics, MoveFunctionName, NetworkMetrics, ParticipationMetrics,
    TransactionFilter,
};
use iota_package_resolver::{Package, PackageStore, PackageStoreWithLruCache, Resolver};
use iota_types::{
    TypeTag,
    balance::Supply,
    base_types::{IotaAddress, ObjectID, SequenceNumber, VersionNumber},
    coin::{CoinMetadata, TreasuryCap},
    committee::EpochId,
    digests::{ChainIdentifier, TransactionDigest},
    dynamic_field::{DynamicFieldInfo, DynamicFieldName, visitor as DFV},
    effects::TransactionEvents,
    event::EventID,
    iota_system_state::{
        IotaSystemStateTrait,
        iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
    },
    is_system_package,
    messages_checkpoint::CheckpointDigest,
    object::{Object, ObjectRead, PastObjectRead, bounded_visitor::BoundedVisitor},
};
use itertools::Itertools;
use move_core_types::{annotated_value::MoveStructLayout, language_storage::StructTag};
use tap::TapFallible;

use crate::{
    db::{ConnectionConfig, ConnectionPool, ConnectionPoolConfig},
    errors::IndexerError,
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
        transactions::{
            OptimisticTransaction, StoredTransaction, StoredTransactionEvents,
            stored_events_to_events, tx_events_to_iota_tx_events,
        },
        tx_indices::TxSequenceNumber,
    },
    schema::{
        address_metrics, addresses, chain_identifier, checkpoints, display, epochs, events,
        objects, objects_history, objects_snapshot, objects_version, optimistic_transactions,
        packages, pruner_cp_watermark, transactions, tx_digests, tx_insertion_order,
    },
    store::{diesel_macro::*, package_resolver::IndexerStorePackageResolver},
    types::{IndexerResult, OwnerType},
};

pub const TX_SEQUENCE_NUMBER_STR: &str = "tx_sequence_number";
pub const TRANSACTION_DIGEST_STR: &str = "transaction_digest";
pub const EVENT_SEQUENCE_NUMBER_STR: &str = "event_sequence_number";

pub struct IndexerReader {
    pool: ConnectionPool,
    package_resolver: PackageResolver,
    package_obj_type_cache: Arc<Mutex<SizedCache<String, Option<ObjectID>>>>,
}

impl Clone for IndexerReader {
    fn clone(&self) -> IndexerReader {
        IndexerReader {
            pool: self.pool.clone(),
            package_resolver: self.package_resolver.clone(),
            package_obj_type_cache: self.package_obj_type_cache.clone(),
        }
    }
}

pub type PackageResolver = Arc<Resolver<PackageStoreWithLruCache<IndexerStorePackageResolver>>>;

// Impl for common initialization and utilities
impl IndexerReader {
    pub fn new<T: Into<String>>(db_url: T) -> Result<Self> {
        let config = ConnectionPoolConfig::default();
        Self::new_with_config(db_url, config)
    }

    pub fn new_with_config<T: Into<String>>(
        db_url: T,
        config: ConnectionPoolConfig,
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
            .map_err(|e| anyhow!("Failed to initialize connection pool. Error: {:?}. If Error is None, please check whether the configured pool size (currently {}) exceeds the maximum number of connections allowed by the database.", e, config.pool_size))?;

        let indexer_store_pkg_resolver = IndexerStorePackageResolver::new(pool.clone());
        let package_cache = PackageStoreWithLruCache::new(indexer_store_pkg_resolver);
        let package_resolver = Arc::new(Resolver::new(package_cache));
        let package_obj_type_cache = Arc::new(Mutex::new(SizedCache::with_size(10000)));
        Ok(Self {
            pool,
            package_resolver,
            package_obj_type_cache,
        })
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
            CALLED_FROM_BLOCKING_POOL
                .with(|in_blocking_pool| *in_blocking_pool.borrow_mut() = true);
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
        let object_id = object_id.to_vec();

        let stored_object = run_query!(&self.pool, |conn| {
            if let Some(version) = version {
                objects::dsl::objects
                    .filter(objects::dsl::object_id.eq(object_id))
                    .filter(objects::dsl::object_version.eq(version.value() as i64))
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
            object
                .try_into_object_read(self.package_resolver.clone())
                .await
        } else {
            Ok(ObjectRead::NotExists(object_id))
        }
    }

    fn get_object_raw(&self, object_id: ObjectID) -> Result<Option<StoredObject>, IndexerError> {
        let id = object_id.to_vec();
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
    pub(crate) async fn get_past_object_read(
        &self,
        object_id: ObjectID,
        object_version: SequenceNumber,
        before_version: bool,
    ) -> Result<PastObjectRead, IndexerError> {
        let object_version_num = object_version.value() as i64;

        // Query objects_version to find the requested version and relevant
        // checkpoint sequence number considering the `before_version` flag.
        let pool = self.get_pool();
        let object_id_bytes = object_id.to_vec();
        let object_version_info: Option<StoredObjectVersion> =
            run_query_async!(&pool, move |conn| {
                let mut query = objects_version::dsl::objects_version
                    .filter(objects_version::object_id.eq(&object_id_bytes))
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
            })?;

        let Some(object_version_info) = object_version_info else {
            // Check if the object ever existed.
            let pool = self.get_pool();
            let object_id_bytes = object_id.to_vec();
            let latest_existing_version: Option<i64> = run_query_async!(&pool, move |conn| {
                objects_version::dsl::objects_version
                    .filter(objects_version::object_id.eq(&object_id_bytes))
                    .order_by(objects_version::object_version.desc())
                    .select(objects_version::object_version)
                    .limit(1)
                    .first::<i64>(conn)
                    .optional()
            })?;

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

        // Query objects_history for the object with the requested version.
        let history_object = self
            .get_stored_history_object(
                object_id,
                object_version_info.object_version,
                object_version_info.cp_sequence_number,
            )
            .await?;

        match history_object {
            Some(obj) => {
                obj.try_into_past_object_read(self.package_resolver.clone())
                    .await
            }
            None => Err(IndexerError::PersistentStorageDataCorruption(format!(
                "Object version {} not found in objects_history for object {}",
                object_version_info.object_version, object_id
            ))),
        }
    }

    pub async fn get_stored_history_object(
        &self,
        object_id: ObjectID,
        object_version: i64,
        checkpoint_sequence_number: i64,
    ) -> Result<Option<StoredHistoryObject>, IndexerError> {
        let pool = self.get_pool();
        let object_id_bytes = object_id.to_vec();
        run_query_async!(&pool, move |conn| {
            // Match on the primary key.
            let query = objects_history::dsl::objects_history
                .filter(objects_history::checkpoint_sequence_number.eq(checkpoint_sequence_number))
                .filter(objects_history::object_id.eq(&object_id_bytes))
                .filter(objects_history::object_version.eq(object_version))
                .into_boxed();

            query
                .order_by(objects_history::object_version.desc())
                .limit(1)
                .first::<StoredHistoryObject>(conn)
                .optional()
        })
    }

    pub async fn get_package(&self, package_id: ObjectID) -> Result<Package, IndexerError> {
        let store = self.package_resolver.package_store();
        let pkg = store
            .fetch(package_id.into())
            .await
            .map_err(|e| {
                IndexerError::PostgresRead(format!(
                    "Fail to fetch package from package store with error {:?}",
                    e
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

        let system_state: IotaSystemStateSummary = bcs::from_bytes(&stored_epoch.system_state)
            .map_err(|_| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to deserialize `system_state` for epoch {:?}",
                    epoch,
                ))
            })?;
        Ok(system_state)
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

        let checkpoint_digest =
            CheckpointDigest::try_from(stored_chain_identifier.checkpoint_digest).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "failed to decode chain identifier with err: {e:?}"
                ))
            })?;

        Ok(checkpoint_digest.into())
    }

    pub fn get_checkpoint_from_db(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<Option<StoredCheckpoint>, IndexerError> {
        let stored_checkpoint = run_query!(&self.pool, |conn| {
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

        Ok(stored_checkpoint)
    }

    pub fn get_latest_checkpoint_from_db(&self) -> Result<StoredCheckpoint, IndexerError> {
        let stored_checkpoint = run_query!(&self.pool, |conn| {
            checkpoints::dsl::checkpoints
                .order_by(checkpoints::sequence_number.desc())
                .first::<StoredCheckpoint>(conn)
        })?;

        Ok(stored_checkpoint)
    }

    pub fn get_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<Option<iota_json_rpc_types::Checkpoint>, IndexerError> {
        let stored_checkpoint = match self.get_checkpoint_from_db(checkpoint_id)? {
            Some(stored_checkpoint) => stored_checkpoint,
            None => return Ok(None),
        };

        let checkpoint = iota_json_rpc_types::Checkpoint::try_from(stored_checkpoint)?;
        Ok(Some(checkpoint))
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

    fn get_checkpoints_from_db(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> Result<Vec<StoredCheckpoint>, IndexerError> {
        run_query!(&self.pool, |conn| {
            let mut boxed_query = checkpoints::table.into_boxed();
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

    pub fn get_checkpoints(
        &self,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> Result<Vec<iota_json_rpc_types::Checkpoint>, IndexerError> {
        self.get_checkpoints_from_db(cursor, limit, descending_order)?
            .into_iter()
            .map(iota_json_rpc_types::Checkpoint::try_from)
            .collect()
    }

    fn get_transaction_effects_with_digest(
        &self,
        digest: TransactionDigest,
    ) -> Result<IotaTransactionBlockEffects, IndexerError> {
        let stored_txn: StoredTransaction = run_query!(&self.pool, |conn| {
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
                .first::<StoredTransaction>(conn)
        })?;

        stored_txn.try_into_iota_transaction_effects()
    }

    fn get_transaction_effects_with_sequence_number(
        &self,
        sequence_number: i64,
    ) -> Result<IotaTransactionBlockEffects, IndexerError> {
        let stored_txn: StoredTransaction = run_query!(&self.pool, |conn| {
            transactions::table
                .filter(transactions::tx_sequence_number.eq(sequence_number))
                .first::<StoredTransaction>(conn)
        })?;

        stored_txn.try_into_iota_transaction_effects()
    }

    fn multi_get_transactions(
        &self,
        digests: &[TransactionDigest],
    ) -> Result<Vec<StoredTransaction>, IndexerError> {
        let digests = digests
            .iter()
            .map(|digest| digest.inner().to_vec())
            .collect::<HashSet<_>>();
        let checkpointed_txs = run_query!(&self.pool, |conn| {
            transactions::table
                .inner_join(
                    tx_digests::table
                        .on(transactions::tx_sequence_number.eq(tx_digests::tx_sequence_number)),
                )
                // we filter the tx_digests table because it is indexed by digest,
                // transactions table is not
                .filter(tx_digests::tx_digest.eq_any(&digests))
                .select(StoredTransaction::as_select())
                .load::<StoredTransaction>(conn)
        })?;
        if checkpointed_txs.len() == digests.len() {
            return Ok(checkpointed_txs);
        }
        let mut missing_digests = digests;
        for tx in &checkpointed_txs {
            missing_digests.remove(&tx.transaction_digest);
        }
        let optimistic_txs = run_query!(&self.pool, |conn| {
            optimistic_transactions::table
                .inner_join(
                    tx_insertion_order::table.on(optimistic_transactions::insertion_order
                        .eq(tx_insertion_order::insertion_order)),
                )
                // we filter the tx_insertion_order table because it is indexed by digest,
                // optimistic_transactions table is not
                .filter(tx_insertion_order::tx_digest.eq_any(missing_digests))
                .select(OptimisticTransaction::as_select())
                .load::<OptimisticTransaction>(conn)
        })?;
        Ok(checkpointed_txs
            .into_iter()
            .chain(optimistic_txs.into_iter().map(Into::into))
            .collect())
    }

    async fn multi_get_transactions_in_blocking_task(
        &self,
        digests: Vec<TransactionDigest>,
    ) -> Result<Vec<StoredTransaction>, IndexerError> {
        self.spawn_blocking(move |this| this.multi_get_transactions(&digests))
            .await
    }

    /// This method tries to transform [`StoredTransaction`] values
    /// into transaction blocks, without any other modification.
    async fn stored_transaction_to_transaction_block(
        &self,
        stored_txes: Vec<StoredTransaction>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        let mut tx_block_responses_futures = vec![];
        for stored_tx in stored_txes {
            let package_resolver_clone = self.package_resolver();
            let options_clone = options.clone();
            tx_block_responses_futures.push(tokio::task::spawn(
                stored_tx.try_into_iota_transaction_block_response(
                    options_clone,
                    package_resolver_clone,
                ),
            ));
        }

        let tx_blocks = futures::future::join_all(tx_block_responses_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("Failed to join all tx block futures: {}", e))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("Failed to collect tx block futures: {}", e))?;
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
                .filter(objects::dsl::owner_id.eq(address.to_vec()))
                .order(objects::dsl::object_id.asc())
                .limit(limit as i64)
                .into_boxed();
            if let Some(filter) = filter {
                match filter {
                    IotaObjectDataFilter::StructType(struct_tag) => {
                        let object_type =
                            struct_tag.to_canonical_string(/* with_prefix */ true);
                        query =
                            query.filter(objects::object_type.like(format!("{}%", object_type)));
                    }
                    IotaObjectDataFilter::MatchAny(filters) => {
                        let mut condition = "(".to_string();
                        for (i, filter) in filters.iter().enumerate() {
                            if let IotaObjectDataFilter::StructType(struct_tag) = filter {
                                let object_type =
                                    struct_tag.to_canonical_string(/* with_prefix */ true);
                                if i == 0 {
                                    condition +=
                                        format!("objects.object_type LIKE '{}%'", object_type)
                                            .as_str();
                                } else {
                                    condition +=
                                        format!(" OR objects.object_type LIKE '{}%'", object_type)
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
                                    objects::object_type.not_like(format!("{}%", object_type)),
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
                query = query.filter(objects::dsl::object_id.gt(object_cursor.to_vec()));
            }

            query
                .load::<StoredObject>(conn)
                .map_err(|e| IndexerError::PostgresRead(e.to_string()))
        })
    }

    fn filter_object_id_with_type(
        &self,
        object_ids: Vec<ObjectID>,
        object_type: String,
    ) -> Result<Vec<ObjectID>, IndexerError> {
        let object_ids = object_ids.into_iter().map(|id| id.to_vec()).collect_vec();
        let filtered_ids = run_query!(&self.pool, |conn| {
            objects::dsl::objects
                .filter(objects::object_id.eq_any(object_ids))
                .filter(objects::object_type.eq(object_type))
                .select(objects::object_id)
                .load::<Vec<u8>>(conn)
        })?;

        filtered_ids
            .into_iter()
            .map(|id| {
                ObjectID::from_bytes(id.clone()).map_err(|_e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Can't convert {:?} to ObjectID",
                        id,
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()
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
        let object_ids = object_ids.into_iter().map(|id| id.to_vec()).collect_vec();
        run_query!(&self.pool, |conn| {
            objects::dsl::objects
                .filter(objects::object_id.eq_any(object_ids))
                .load::<StoredObject>(conn)
        })
    }

    async fn query_transaction_blocks_by_checkpoint_impl(
        &self,
        checkpoint_seq: u64,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor_tx_seq: Option<i64>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        let pool = self.get_pool();
        let tx_range: (i64, i64) = run_query_async!(&pool, move |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select((
                    pruner_cp_watermark::min_tx_sequence_number,
                    pruner_cp_watermark::max_tx_sequence_number,
                ))
                // we filter the pruner_cp_watermark table because it is indexed by
                // checkpoint_sequence_number, transactions is not
                .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(checkpoint_seq as i64))
                .first::<(i64, i64)>(conn)
        })?;

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
        let pool = self.get_pool();
        let stored_txes = run_query_async!(&pool, move |conn| query
            .limit(limit as i64)
            .load::<StoredTransaction>(conn))?;

        self.stored_transaction_to_transaction_block(stored_txes, options)
            .await
    }

    pub async fn query_transaction_blocks_in_blocking_task(
        &self,
        filter: Option<TransactionFilter>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        self.query_transaction_blocks_impl(filter, options, cursor, limit, is_descending)
            .await
    }

    async fn query_transaction_blocks_impl(
        &self,
        filter: Option<TransactionFilter>,
        options: iota_json_rpc_types::IotaTransactionBlockResponseOptions,
        cursor: Option<TransactionDigest>,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<IotaTransactionBlockResponse>> {
        let cursor_tx_seq = if let Some(cursor) = cursor {
            let pool = self.get_pool();
            let tx_seq = run_query_async!(&pool, move |conn| {
                tx_digests::table
                    .select(tx_digests::tx_sequence_number)
                    // we filter the tx_digests table because it is indexed by digest,
                    // transactions (and other tables) are not
                    .filter(tx_digests::tx_digest.eq(cursor.into_inner().to_vec()))
                    .first::<i64>(conn)
            })?;
            Some(tx_seq)
        } else {
            None
        };
        let cursor_clause = if let Some(cursor_tx_seq) = cursor_tx_seq {
            if is_descending {
                format!("AND {TX_SEQUENCE_NUMBER_STR} < {}", cursor_tx_seq)
            } else {
                format!("AND {TX_SEQUENCE_NUMBER_STR} > {}", cursor_tx_seq)
            }
        } else {
            "".to_string()
        };
        let order_str = if is_descending { "DESC" } else { "ASC" };
        let (table_name, main_where_clause) = match filter {
            // Processed above
            Some(TransactionFilter::Checkpoint(seq)) => {
                return self
                    .query_transaction_blocks_by_checkpoint_impl(
                        seq,
                        options,
                        cursor_tx_seq,
                        limit,
                        is_descending,
                    )
                    .await;
            }
            // FIXME: sanitize module & function
            Some(TransactionFilter::MoveFunction {
                package,
                module,
                function,
            }) => {
                let package = Hex::encode(package.to_vec());
                match (module, function) {
                    (Some(module), Some(function)) => (
                        "tx_calls_fun".into(),
                        format!(
                            "package = '\\x{}'::bytea AND module = '{}' AND func = '{}'",
                            package, module, function
                        ),
                    ),
                    (Some(module), None) => (
                        "tx_calls_mod".into(),
                        format!(
                            "package = '\\x{}'::bytea AND module = '{}'",
                            package, module
                        ),
                    ),
                    (None, Some(_)) => {
                        return Err(IndexerError::InvalidArgument(
                            "Function cannot be present without Module.".into(),
                        ));
                    }
                    (None, None) => (
                        "tx_calls_pkg".into(),
                        format!("package = '\\x{}'::bytea", package),
                    ),
                }
            }
            Some(TransactionFilter::InputObject(object_id)) => {
                let object_id = Hex::encode(object_id.to_vec());
                (
                    "tx_input_objects".into(),
                    format!("object_id = '\\x{}'::bytea", object_id),
                )
            }
            Some(TransactionFilter::ChangedObject(object_id)) => {
                let object_id = Hex::encode(object_id.to_vec());
                (
                    "tx_changed_objects".into(),
                    format!("object_id = '\\x{}'::bytea", object_id),
                )
            }
            Some(TransactionFilter::FromAddress(from_address)) => {
                let from_address = Hex::encode(from_address.to_vec());
                (
                    "tx_senders".into(),
                    format!("sender = '\\x{}'::bytea", from_address),
                )
            }
            Some(TransactionFilter::ToAddress(to_address)) => {
                let to_address = Hex::encode(to_address.to_vec());
                (
                    "tx_recipients".into(),
                    format!("recipient = '\\x{}'::bytea", to_address),
                )
            }
            Some(TransactionFilter::FromAndToAddress { from, to }) => {
                let from_address = Hex::encode(from.to_vec());
                let to_address = Hex::encode(to.to_vec());
                // Need to remove ambiguities for tx_sequence_number column
                let cursor_clause = if let Some(cursor_tx_seq) = cursor_tx_seq {
                    if is_descending {
                        format!(
                            "AND tx_senders.{TX_SEQUENCE_NUMBER_STR} < {}",
                            cursor_tx_seq
                        )
                    } else {
                        format!(
                            "AND tx_senders.{TX_SEQUENCE_NUMBER_STR} > {}",
                            cursor_tx_seq
                        )
                    }
                } else {
                    "".to_string()
                };
                let inner_query = format!(
                    "(SELECT tx_senders.{TX_SEQUENCE_NUMBER_STR} \
                    FROM tx_senders \
                    JOIN tx_recipients \
                    ON tx_senders.{TX_SEQUENCE_NUMBER_STR} = tx_recipients.{TX_SEQUENCE_NUMBER_STR} \
                    WHERE tx_senders.sender = '\\x{}'::BYTEA \
                    AND tx_recipients.recipient = '\\x{}'::BYTEA \
                    {} \
                    ORDER BY {TX_SEQUENCE_NUMBER_STR} {} \
                    LIMIT {}) AS inner_query
                    ",
                    from_address,
                    to_address,
                    cursor_clause,
                    order_str,
                    limit,
                );
                (inner_query, "1 = 1".into())
            }
            Some(TransactionFilter::FromOrToAddress { addr }) => {
                let address = Hex::encode(addr.to_vec());
                let inner_query = format!(
                    "( \
                        ( \
                            SELECT {TX_SEQUENCE_NUMBER_STR} FROM tx_senders \
                            WHERE sender = '\\x{}'::BYTEA {} \
                            ORDER BY {TX_SEQUENCE_NUMBER_STR} {} \
                            LIMIT {} \
                        ) \
                        UNION \
                        ( \
                            SELECT {TX_SEQUENCE_NUMBER_STR} FROM tx_recipients \
                            WHERE recipient = '\\x{}'::BYTEA {} \
                            ORDER BY {TX_SEQUENCE_NUMBER_STR} {} \
                            LIMIT {} \
                        ) \
                    ) AS combined",
                    address,
                    cursor_clause,
                    order_str,
                    limit,
                    address,
                    cursor_clause,
                    order_str,
                    limit,
                );
                (inner_query, "1 = 1".into())
            }
            Some(TransactionFilter::TransactionKind(kind)) => {
                // The `SystemTransaction` variant can be used to filter for all types of system
                // transactions.
                if kind == IotaTransactionKind::SystemTransaction {
                    ("tx_kinds".into(), "tx_kind != 1".to_string())
                } else {
                    ("tx_kinds".into(), format!("tx_kind = {}", kind as u8))
                }
            }
            Some(TransactionFilter::TransactionKindIn(kind_vec)) => {
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
            None => {
                // apply no filter
                ("transactions".into(), "1 = 1".into())
            }
        };

        let query = format!(
            "SELECT {TX_SEQUENCE_NUMBER_STR} FROM {} WHERE {} {} ORDER BY {TX_SEQUENCE_NUMBER_STR} {} LIMIT {}",
            table_name, main_where_clause, cursor_clause, order_str, limit,
        );

        tracing::debug!("query transaction blocks: {}", query);
        let pool = self.get_pool();
        let tx_sequence_numbers = run_query_async!(&pool, move |conn| {
            diesel::sql_query(query.clone()).load::<TxSequenceNumber>(conn)
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
        let stored_txes = self
            .multi_get_transactions_in_blocking_task(digests.to_vec())
            .await?;
        self.stored_transaction_to_transaction_block(stored_txes, options)
            .await
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

    pub async fn get_transaction_events_in_blocking_task(
        &self,
        digest: TransactionDigest,
    ) -> Result<Vec<iota_json_rpc_types::IotaEvent>, IndexerError> {
        let pool = self.get_pool();
        let (timestamp_ms, serialized_events) = run_query_async!(&pool, move |conn| {
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
        })?;

        let events = stored_events_to_events(serialized_events)?;
        let tx_events = TransactionEvents { data: events };

        let iota_tx_events = tx_events_to_iota_tx_events(
            tx_events,
            self.package_resolver(),
            digest,
            timestamp_ms as u64,
        )
        .await?;
        Ok(iota_tx_events.map_or(vec![], |ste| ste.data))
    }

    async fn query_events_by_tx_digest(
        &self,
        tx_digest: TransactionDigest,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IotaEvent>> {
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

        let pool = self.get_pool();
        let stored_events = run_query_async!(&pool, move |conn| {
            query.limit(limit as i64).load::<StoredEvent>(conn)
        })?;

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
            .tap_err(|e| tracing::error!("Failed to join iota event futures: {}", e))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("Failed to collect iota event futures: {}", e))?;
        Ok(iota_events)
    }

    pub async fn query_events_in_blocking_task(
        &self,
        filter: EventFilter,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IotaEvent>> {
        let pool = self.get_pool();
        let (tx_seq, event_seq) = if let Some(cursor) = cursor {
            let EventID {
                tx_digest,
                event_seq,
            } = cursor;
            let tx_seq = run_query_async!(&pool, move |conn| {
                transactions::dsl::transactions
                    .select(transactions::tx_sequence_number)
                    .filter(
                        transactions::tx_sequence_number
                            .nullable()
                            .eq(tx_digests::table
                                .select(tx_digests::tx_sequence_number)
                                // we filter the tx_digests table because it is indexed by digest,
                                // transactions table is not
                                .filter(tx_digests::tx_digest.eq(tx_digest.into_inner().to_vec()))
                                .single_value()),
                    )
                    .first::<i64>(conn)
            })?;
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
                    "(e.{TX_SEQUENCE_NUMBER_STR} < {} OR (e.{TX_SEQUENCE_NUMBER_STR} = {} AND e.{EVENT_SEQUENCE_NUMBER_STR} < {}))",
                    tx_seq, tx_seq, event_seq
                )
            } else {
                format!(
                    "(e.{TX_SEQUENCE_NUMBER_STR} > {} OR (e.{TX_SEQUENCE_NUMBER_STR} = {} AND e.{EVENT_SEQUENCE_NUMBER_STR} > {}))",
                    tx_seq, tx_seq, event_seq
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
                    WHERE {} \
                    ORDER BY {} \
                    LIMIT {}
                )",
                Hex::encode(sender.to_vec()),
                cursor_clause,
                order_clause,
                limit,
            )
        } else if let EventFilter::Transaction(tx_digest) = filter {
            return self
                .query_events_by_tx_digest(tx_digest, cursor, limit, descending_order)
                .await;
        } else {
            let main_where_clause = match filter {
                EventFilter::Package(package_id) => {
                    format!("package = '\\x{}'::bytea", package_id.to_hex())
                }
                EventFilter::MoveModule { package, module } => {
                    format!(
                        "package = '\\x{}'::bytea AND module = '{}'",
                        package.to_hex(),
                        module,
                    )
                }
                EventFilter::MoveEventType(struct_tag) => {
                    let formatted_struct_tag = struct_tag.to_canonical_string(true);
                    format!("event_type = '{formatted_struct_tag}'")
                }
                EventFilter::MoveEventModule { package, module } => {
                    let package_module_prefix = format!("{}::{}", package.to_hex_literal(), module);
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
                    "AND ({TX_SEQUENCE_NUMBER_STR} < {} OR ({TX_SEQUENCE_NUMBER_STR} = {} AND {EVENT_SEQUENCE_NUMBER_STR} < {}))",
                    tx_seq, tx_seq, event_seq
                )
            } else {
                format!(
                    "AND ({TX_SEQUENCE_NUMBER_STR} > {} OR ({TX_SEQUENCE_NUMBER_STR} = {} AND {EVENT_SEQUENCE_NUMBER_STR} > {}))",
                    tx_seq, tx_seq, event_seq
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
                    WHERE {} {} \
                    ORDER BY {} \
                    LIMIT {}
                ",
                main_where_clause, cursor_clause, order_clause, limit,
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
            .tap_err(|e| tracing::error!("Failed to join iota event futures: {}", e))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("Failed to collect iota event futures: {}", e))?;
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
        let indexer_reader_arc = Arc::new(self.clone());
        for stored_object in stored_objects {
            let indexer_reader_arc_clone = Arc::clone(&indexer_reader_arc);
            df_futures.push(tokio::task::spawn(async move {
                indexer_reader_arc_clone
                    .try_create_dynamic_field_info(stored_object)
                    .await
            }));
        }
        let df_infos = futures::future::try_join_all(df_futures)
            .await
            .tap_err(|e| tracing::error!("Error joining DF futures: {:?}", e))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| {
                tracing::error!(
                    "Error calling DF try_create_dynamic_field_info function: {:?}",
                    e
                )
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
                .filter(objects::dsl::owner_id.eq(parent_object_id.to_vec()))
                .order(objects::dsl::object_id.asc())
                .limit(limit as i64)
                .into_boxed();
            if let Some(object_cursor) = cursor {
                query = query.filter(objects::dsl::object_id.gt(object_cursor.to_vec()));
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
        let Some(move_object) = object.data.try_as_move().cloned() else {
            return Err(IndexerError::ResolveMoveStruct(
                "Object is not a MoveObject".to_string(),
            ));
        };
        let type_tag: TypeTag = move_object.type_().clone().into();
        let layout = self
            .package_resolver
            .type_layout(type_tag.clone())
            .await
            .map_err(|e| {
                IndexerError::ResolveMoveStruct(format!(
                    "Failed to get type layout for type {}: {e}",
                    type_tag.to_canonical_display(/* with_prefix */ true),
                ))
            })?;

        let field = DFV::FieldVisitor::deserialize(move_object.contents(), &layout)
            .tap_err(|e| tracing::warn!("{e}"))?;

        let type_ = field.kind;
        let name_type: TypeTag = field.name_layout.into();
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
                            object_id.to_canonical_display(/* with_prefix */ true),
                        ))
                    })?;

                let object_type = object.data.type_().unwrap().clone();
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
        object_type: &move_core_types::language_storage::StructTag,
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
            .filter(objects::dsl::owner_id.eq(owner.to_vec()))
            .filter(objects::dsl::object_id.gt(cursor.to_vec()))
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
            format!("= '{}'", coin_type)
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
            Hex::encode(owner.to_vec()),
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
        let package_id = coin_struct.address.into();
        let coin_metadata_type =
            CoinMetadata::type_(coin_struct).to_canonical_string(/* with_prefix */ true);
        let coin_metadata_obj_id = *self
            .package_obj_type_cache
            .lock()
            .unwrap()
            .cache_get_or_set_with(format!("{}{}", package_id, coin_metadata_type), || {
                get_single_obj_id_from_package_publish(self, package_id, coin_metadata_type.clone())
                    .unwrap()
            });
        if let Some(id) = coin_metadata_obj_id {
            let metadata_object = self.get_object(&id, None)?;
            Ok(metadata_object.and_then(|v| IotaCoinMetadata::try_from(v).ok()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_total_supply_in_blocking_task(
        &self,
        coin_struct: StructTag,
    ) -> Result<Supply, IndexerError> {
        self.spawn_blocking(move |this| this.get_total_supply(coin_struct))
            .await
    }

    fn get_total_supply(&self, coin_struct: StructTag) -> Result<Supply, IndexerError> {
        let package_id = coin_struct.address.into();
        let treasury_cap_type =
            TreasuryCap::type_(coin_struct).to_canonical_string(/* with_prefix */ true);
        let treasury_cap_obj_id = self
            .package_obj_type_cache
            .lock()
            .unwrap()
            .cache_get_or_set_with(format!("{}{}", package_id, treasury_cap_type), || {
                get_single_obj_id_from_package_publish(self, package_id, treasury_cap_type.clone())
                    .unwrap()
            })
            .ok_or(IndexerError::Generic(format!(
                "Cannot find treasury cap for type {}",
                treasury_cap_type
            )))?;
        let treasury_cap_obj_object =
            self.get_object(&treasury_cap_obj_id, None)?
                .ok_or(IndexerError::Generic(format!(
                    "Cannot find treasury cap object with id {}",
                    treasury_cap_obj_id
                )))?;
        Ok(TreasuryCap::try_from(treasury_cap_obj_object)?.total_supply)
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

    pub fn package_resolver(&self) -> PackageResolver {
        self.package_resolver.clone()
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
    fn get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<iota_types::object::Object>, iota_types::storage::error::Error> {
        self.get_object(object_id, None)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: iota_types::base_types::VersionNumber,
    ) -> Result<Option<iota_types::object::Object>, iota_types::storage::error::Error> {
        self.get_object(object_id, Some(version))
            .map_err(iota_types::storage::error::Error::custom)
    }
}

fn get_single_obj_id_from_package_publish(
    reader: &IndexerReader,
    package_id: ObjectID,
    obj_type: String,
) -> Result<Option<ObjectID>, IndexerError> {
    let publish_txn_effects_opt = if is_system_package(package_id) {
        Some(reader.get_transaction_effects_with_sequence_number(0))
    } else {
        reader.get_object(&package_id, None)?.map(|o| {
            let publish_txn_digest = o.previous_transaction;
            reader.get_transaction_effects_with_digest(publish_txn_digest)
        })
    };
    if let Some(publish_txn_effects) = publish_txn_effects_opt {
        let created_objs = publish_txn_effects?
            .created()
            .iter()
            .map(|o| o.object_id())
            .collect::<Vec<_>>();
        let obj_ids_with_type =
            reader.filter_object_id_with_type(created_objs, obj_type.clone())?;
        if obj_ids_with_type.len() == 1 {
            Ok(Some(obj_ids_with_type[0]))
        } else if obj_ids_with_type.is_empty() {
            // The package exists but no such object is created in that transaction. Or
            // maybe it is wrapped and we don't know yet.
            Ok(None)
        } else {
            // We expect there to be only one object of this type created by the package but
            // more than one is found.
            tracing::error!(
                "There are more than one objects found for type {}",
                obj_type
            );
            Ok(None)
        }
    } else {
        // The coin package does not exist.
        Ok(None)
    }
}
