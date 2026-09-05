// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use iota_core::authority::AuthorityState;
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{
    IndexerApiOpenRpc, IndexerApiServer, JsonRpcMetrics, QUERY_MAX_RESULT_LIMIT, ReadApiServer,
    cap_page_limit, validate_limit,
};
use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaNameRecord, IotaObjectDataFilter,
    IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseError, IotaObjectResponseQuery,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseQuery,
    IotaTransactionBlockResponseQueryV2, ObjectsPage, OwnedObjectCursor, Page,
    TransactionBlocksPage,
};
use iota_metrics::spawn_monitored_blocking;
use iota_names::{
    IotaNamesNft, NameRegistration, config::IotaNamesConfig, error::IotaNamesError, name::Name,
    registry::NameRecord,
};
use iota_open_rpc::Module;
use iota_sdk_move_types::iota_framework::dynamic_field::Field;
use iota_sdk_types::{Address, ObjectId, TransactionDigest, TypeTag};
use iota_storage::key_value_store::TransactionKeyValueStore;
use iota_types::{
    dynamic_field::DynamicFieldName, error::UserInputError, event::EventID,
    iota_sdk_types_conversions::type_tag_sdk_to_core, storage::OwnedObjectCursor as IndexCursor,
};
use jsonrpsee::{RpcModule, core::RpcResult};
use move_bytecode_utils::layout::TypeLayoutBuilder;
use tracing::instrument;

use crate::{
    IotaRpcModule,
    authority_state::{StateRead, StateReadResult},
    error::{Error, IotaRpcInputError},
    logger::FutureWithTracing as _,
};

pub struct IndexerApi<R> {
    state: Arc<dyn StateRead>,
    read_api: R,
    transaction_kv_store: Arc<TransactionKeyValueStore>,
    iota_names_config: IotaNamesConfig,
    pub metrics: Arc<JsonRpcMetrics>,
}

impl<R: ReadApiServer> IndexerApi<R> {
    pub fn new(
        state: Arc<AuthorityState>,
        read_api: R,
        transaction_kv_store: Arc<TransactionKeyValueStore>,
        metrics: Arc<JsonRpcMetrics>,
        iota_names_config: IotaNamesConfig,
    ) -> Self {
        Self {
            state,
            transaction_kv_store,
            read_api,
            metrics,
            iota_names_config,
        }
    }

    fn extract_values_from_dynamic_field_name(
        &self,
        name: DynamicFieldName,
    ) -> Result<(TypeTag, Vec<u8>), IotaRpcInputError> {
        let DynamicFieldName {
            type_tag: name_type,
            value,
        } = name;
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let layout = TypeLayoutBuilder::build_with_types(
            &type_tag_sdk_to_core(&name_type),
            epoch_store.module_cache(),
        )?;
        let iota_json_value = IotaJsonValue::new(value)?;
        let name_bcs_value = iota_json_value.to_bcs_bytes(&layout)?;
        Ok((name_type, name_bcs_value))
    }

    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectId,
        name: DynamicFieldName,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        async move {
            let (name_type, name_bcs_value) = self.extract_values_from_dynamic_field_name(name)?;

            let id = self
                .state
                .get_dynamic_field_object_id(parent_object_id, name_type, &name_bcs_value)
                .map_err(Error::from)?;

            if let Some(id) = id {
                self.read_api
                    .get_object(id, options)
                    .await
                    .map_err(|e| Error::Internal(anyhow!(e)))
            } else {
                Ok(IotaObjectResponse::new_with_error(
                    IotaObjectResponseError::DynamicFieldNotFound { parent_object_id },
                ))
            }
        }
        .trace()
        .await
    }

    fn get_latest_checkpoint_timestamp_ms(&self) -> StateReadResult<u64> {
        let latest_checkpoint = self.state.get_latest_checkpoint_sequence_number()?;

        let checkpoint = self
            .state
            .get_verified_checkpoint_by_sequence_number(latest_checkpoint)?;

        Ok(checkpoint.timestamp_ms)
    }
}

#[async_trait]
impl<R: ReadApiServer> IndexerApiServer for IndexerApi<R> {
    #[instrument(skip(self, address), fields(address = %address))]
    async fn get_owned_objects(
        &self,
        address: Address,
        query: Option<IotaObjectResponseQuery>,
        cursor: Option<OwnedObjectCursor>,
        limit: Option<usize>,
    ) -> RpcResult<ObjectsPage> {
        async move {
            let limit =
                validate_limit(limit, *QUERY_MAX_RESULT_LIMIT).map_err(IotaRpcInputError::from)?;
            self.metrics.get_owned_objects_limit.observe(limit as f64);
            let IotaObjectResponseQuery { filter, options } = query.unwrap_or_default();
            if let Some(filter) = &filter {
                filter.check_size().map_err(|error| {
                    IotaRpcInputError::GenericInvalid(format!("invalid object filter: {error}"))
                })?;
            }
            let options = options.unwrap_or_default();
            let cursor = cursor_position(cursor)?;
            let state = self.state.clone();
            let page = spawn_monitored_blocking!(move || {
                state.get_owner_objects_page(address, cursor, limit, filter)
            })
            .await??;

            let data = match options.is_not_in_object_info() {
                true => {
                    let object_ids = page.data.iter().map(|obj| obj.object_id).collect();
                    self.read_api
                        .multi_get_objects(object_ids, Some(options))
                        .await
                        .map_err(|e| Error::Internal(anyhow!(e)))?
                }
                false => page
                    .data
                    .into_iter()
                    .map(|o_info| IotaObjectResponse::try_from((o_info, options.clone())))
                    .collect::<Result<Vec<IotaObjectResponse>, _>>()?,
            };

            self.metrics
                .get_owned_objects_result_size
                .observe(data.len() as f64);
            self.metrics
                .get_owned_objects_result_size_total
                .inc_by(data.len() as u64);
            Ok(Page {
                data,
                next_cursor: page.next_cursor,
                has_next_page: page.has_next_page,
                oldest_available_checkpoint: None,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn query_transaction_blocks(
        &self,
        query: IotaTransactionBlockResponseQuery,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<TransactionBlocksPage> {
        async move {
            let limit = cap_page_limit(limit);
            self.metrics.query_tx_blocks_limit.observe(limit as f64);
            let descending = descending_order.unwrap_or_default();
            let opts = query.options.unwrap_or_default();

            // Retrieve 1 extra item for next cursor
            let mut digests = self
                .state
                .get_transactions(
                    &self.transaction_kv_store,
                    query.filter,
                    cursor,
                    Some(limit + 1),
                    descending,
                )
                .await
                .map_err(Error::from)?;
            // De-dup digests, duplicate digests are possible, for example,
            // when get_transactions_by_move_function with module or function being None.
            let mut seen = HashSet::new();
            digests.retain(|digest| seen.insert(*digest));

            // extract next cursor
            let has_next_page = digests.len() > limit;
            digests.truncate(limit);
            let next_cursor = digests.last().cloned().map_or(cursor, Some);

            let data: Vec<IotaTransactionBlockResponse> = if opts.only_digest() {
                digests
                    .into_iter()
                    .map(IotaTransactionBlockResponse::new)
                    .collect()
            } else {
                self.read_api
                    .multi_get_transaction_blocks(digests, Some(opts))
                    .await
                    .map_err(|e| Error::Internal(anyhow!(e)))?
            };

            self.metrics
                .query_tx_blocks_result_size
                .observe(data.len() as f64);
            self.metrics
                .query_tx_blocks_result_size_total
                .inc_by(data.len() as u64);
            Ok(Page {
                data,
                next_cursor,
                has_next_page,
                oldest_available_checkpoint: None,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn query_transaction_blocks_v2(
        &self,
        query: IotaTransactionBlockResponseQueryV2,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<TransactionBlocksPage> {
        let v1_filter = query
            .filter
            .map(|f| {
                f.as_v1().ok_or_else(|| {
                    Error::UserInput(UserInputError::Unsupported(
                        "transaction filter is not supported".to_string(),
                    ))
                })
            })
            .transpose()?;

        let v1_query = IotaTransactionBlockResponseQuery {
            filter: v1_filter,
            options: query.options,
        };
        self.query_transaction_blocks(v1_query, cursor, limit, descending_order)
            .await
    }

    #[instrument(skip(self))]
    async fn query_events(
        &self,
        query: EventFilter,
        // exclusive cursor if `Some`, otherwise start from the beginning
        cursor: Option<EventID>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<EventPage> {
        async move {
            let descending = descending_order.unwrap_or_default();
            let limit = cap_page_limit(limit);
            self.metrics.query_events_limit.observe(limit as f64);
            // Retrieve 1 extra item for next cursor
            let mut data = self
                .state
                .query_events(
                    &self.transaction_kv_store,
                    query,
                    cursor,
                    limit + 1,
                    descending,
                )
                .await
                .map_err(Error::from)?;
            let has_next_page = data.len() > limit;
            data.truncate(limit);
            let next_cursor = data.last().map_or(cursor, |e| Some(e.id));
            self.metrics
                .query_events_result_size
                .observe(data.len() as f64);
            self.metrics
                .query_events_result_size_total
                .inc_by(data.len() as u64);
            Ok(EventPage {
                data,
                next_cursor,
                has_next_page,
                oldest_available_checkpoint: None,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self, parent_object_id), fields(parent_object_id = %parent_object_id))]
    async fn get_dynamic_fields(
        &self,
        parent_object_id: ObjectId,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectId>,
        limit: Option<usize>,
    ) -> RpcResult<DynamicFieldPage> {
        async move {
            let limit = cap_page_limit(limit);
            self.metrics.get_dynamic_fields_limit.observe(limit as f64);
            // An unresolvable field (`None` info) advances the cursor but is
            // omitted from the page. A page with none resolved is retried a
            // bounded number of times, since clients may treat an empty page
            // as the end.
            const MAX_EMPTY_PAGE_RETRIES: usize = 3;
            let mut next_cursor = cursor;
            let mut has_next_page;
            let mut resolved;
            let mut retries = 0;
            loop {
                let mut data = self
                    .state
                    .get_dynamic_fields(parent_object_id, next_cursor, limit + 1)
                    .map_err(Error::from)?;
                has_next_page = data.len() > limit;
                data.truncate(limit);
                next_cursor = data.last().map_or(next_cursor, |c| Some(c.0));
                resolved = data
                    .into_iter()
                    .filter_map(|(_, info)| Some(info?.into()))
                    .collect::<Vec<_>>();
                if !resolved.is_empty() || !has_next_page || retries >= MAX_EMPTY_PAGE_RETRIES {
                    break;
                }
                retries += 1;
            }
            self.metrics
                .get_dynamic_fields_result_size
                .observe(resolved.len() as f64);
            self.metrics
                .get_dynamic_fields_result_size_total
                .inc_by(resolved.len() as u64);
            Ok(DynamicFieldPage {
                data: resolved,
                next_cursor,
                has_next_page,
                oldest_available_checkpoint: None,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self, parent_object_id), fields(parent_object_id = %parent_object_id))]
    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectId,
        name: DynamicFieldName,
    ) -> RpcResult<IotaObjectResponse> {
        self.get_dynamic_field_object(
            parent_object_id,
            name,
            Some(IotaObjectDataOptions::full_content()),
        )
        .await
    }

    #[instrument(skip(self, parent_object_id), fields(parent_object_id = %parent_object_id))]
    async fn get_dynamic_field_object_v2(
        &self,
        parent_object_id: ObjectId,
        name: DynamicFieldName,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        self.get_dynamic_field_object(parent_object_id, name, options)
            .await
    }

    async fn iota_names_lookup(&self, name: &str) -> RpcResult<Option<IotaNameRecord>> {
        let name = name.parse::<Name>().map_err(Error::from)?;

        // Construct the record id to lookup.
        let record_id = self.iota_names_config.record_field_id(&name);

        let parent_record_id = name
            .parent()
            .map(|parent_name| self.iota_names_config.record_field_id(&parent_name));

        // Keep record IDs alive by declaring both before creating futures
        let mut requests = vec![self.state.get_object(&record_id)];

        // We only want to fetch both the child and the parent if the name is a
        // subname.
        if let Some(ref parent_record_id) = parent_record_id {
            requests.push(self.state.get_object(parent_record_id));
        }

        // Couldn't find a `multi_get_object` for this crate (looks like it uses a k,v
        // db) Always fetching both parent + child at the same time (even for
        // node subnames), to avoid sequential db reads. We do this because we
        // do not know if the requested name is a node subname or a leaf
        // subname, and we can save a trip to the db.
        let mut results = futures::future::try_join_all(requests)
            .await
            .map_err(Error::from)?;

        // Removing without checking vector len, since it is known (== 1 or 2 depending
        // on whether it is a subname or not).
        let Some(object) = results.remove(0) else {
            return Ok(None);
        };

        let name_record = NameRecord::try_from(object).map_err(Error::from)?;

        let current_timestamp_ms = self
            .get_latest_checkpoint_timestamp_ms()
            .map_err(Error::from)?;

        // Handling second-level names & node subnames is the same (we handle them as
        // `node` records). We check their expiration, and if not expired,
        // return the target address.
        if !name_record.is_leaf_record() {
            return if !name_record.is_node_expired(current_timestamp_ms) {
                Ok(Some(name_record.into()))
            } else {
                Err(Error::from(IotaNamesError::NameExpired).into())
            };
        } else {
            // Handle the `leaf` record case which requires to check the parent for
            // expiration. We can remove since we know that if we're here, we have a parent
            // result for the parent request. If the parent result is `None` for the
            // existing leaf record, we consider it expired.
            let Some(parent_object) = results.remove(0) else {
                return Err(Error::from(IotaNamesError::NameExpired).into());
            };

            let parent_name_record = NameRecord::try_from(parent_object).map_err(Error::from)?;

            // For a leaf record, we check that:
            // 1. The parent is a valid parent for that leaf record
            // 2. The parent is not expired
            if parent_name_record.is_valid_leaf_parent(&name_record)
                && !parent_name_record.is_node_expired(current_timestamp_ms)
            {
                Ok(Some(name_record.into()))
            } else {
                Err(Error::from(IotaNamesError::NameExpired).into())
            }
        }
    }

    #[instrument(skip(self, address), fields(address = %address))]
    async fn iota_names_reverse_lookup(&self, address: Address) -> RpcResult<Option<String>> {
        let reverse_record_id = self.iota_names_config.reverse_record_field_id(&address);

        let Some(field_reverse_record_object) = self
            .state
            .get_object(&reverse_record_id)
            .await
            .map_err(Error::from)?
        else {
            return Ok(None);
        };

        let name = field_reverse_record_object
            .to_rust::<Field<Address, Name>>()
            .map_err(|e| Error::Unexpected(format!("malformed Object {reverse_record_id}: {e}")))?
            .value;

        let name = name.to_string();

        let resolved_record = self.iota_names_lookup(&name).await?;

        // If looking up the name returns an empty result, we return an empty result.
        if resolved_record.is_none() {
            return Ok(None);
        }

        Ok(Some(name))
    }

    #[instrument(skip(self, address), fields(address = %address))]
    async fn iota_names_find_all_registration_nfts(
        &self,
        address: Address,
        cursor: Option<OwnedObjectCursor>,
        limit: Option<usize>,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<ObjectsPage> {
        let query = IotaObjectResponseQuery {
            filter: Some(IotaObjectDataFilter::StructType(
                NameRegistration::struct_tag(self.iota_names_config.package_address),
            )),
            options,
        };

        let owned_objects = self
            .get_owned_objects(address, Some(query), cursor, limit)
            .await?;

        Ok(owned_objects)
    }
}

impl<R: ReadApiServer> IotaRpcModule for IndexerApi<R> {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        IndexerApiOpenRpc::module_doc()
    }
}

#[cfg(test)]
mod tests {
    use iota_json_rpc_types::MAX_FILTER_NODES;
    use iota_sdk_types::{CheckpointDigest, TransactionEvents, Version};
    use iota_storage::{
        key_value_store::{
            KVStoreCheckpointData, KVStoreTransactionData, TransactionKeyValueStoreTrait,
        },
        key_value_store_metrics::KeyValueStoreMetrics,
    };
    use iota_types::{
        error::IotaResult, messages_checkpoint::CheckpointSequenceNumber, object::Object,
        storage::ObjectKey,
    };
    use jsonrpsee::{core::server::MethodsError, types::error::INVALID_PARAMS_CODE};
    use mockall::mock;

    use super::*;
    use crate::{authority_state::MockStateRead, read_api::ReadApi};

    mock! {
        KeyValueStore {}
        #[async_trait]
        impl TransactionKeyValueStoreTrait for KeyValueStore {
            async fn multi_get(
                &self,
                transaction_keys: &[TransactionDigest],
                effects_keys: &[TransactionDigest],
            ) -> IotaResult<KVStoreTransactionData>;
            async fn multi_get_checkpoints(
                &self,
                checkpoint_summaries: &[CheckpointSequenceNumber],
                checkpoint_contents: &[CheckpointSequenceNumber],
                checkpoint_summaries_by_digest: &[CheckpointDigest],
            ) -> IotaResult<KVStoreCheckpointData>;
            async fn get_transaction_perpetual_checkpoint(
                &self,
                digest: TransactionDigest,
            ) -> IotaResult<Option<CheckpointSequenceNumber>>;
            async fn get_object(
                &self,
                object_id: ObjectId,
                version: Version,
            ) -> IotaResult<Option<Object>>;
            async fn multi_get_objects(
                &self,
                object_keys: &[ObjectKey],
            ) -> IotaResult<Vec<Option<Object>>>;
            async fn multi_get_transactions_perpetual_checkpoints(
                &self,
                digests: &[TransactionDigest],
            ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>>;
            async fn multi_get_events_by_tx_digests(
                &self,
                digests: &[TransactionDigest],
            ) -> IotaResult<Vec<Option<TransactionEvents>>>;
        }
    }

    fn indexer_rpc_module() -> RpcModule<IndexerApi<ReadApi>> {
        let state: Arc<dyn StateRead> = Arc::new(MockStateRead::new());
        let transaction_kv_store = Arc::new(TransactionKeyValueStore::new(
            "test",
            KeyValueStoreMetrics::new_for_tests(),
            Arc::new(MockKeyValueStore::new()),
        ));
        let metrics = Arc::new(JsonRpcMetrics::new_for_tests());
        IndexerApi {
            read_api: ReadApi {
                state: state.clone(),
                transaction_kv_store: transaction_kv_store.clone(),
                metrics: metrics.clone(),
            },
            state,
            transaction_kv_store,
            metrics,
            iota_names_config: IotaNamesConfig::default(),
        }
        .rpc()
    }

    fn assert_invalid_params(result: Result<impl std::fmt::Debug, MethodsError>, limit: &str) {
        match result {
            Err(MethodsError::JsonRpc(error)) => {
                assert_eq!(error.code(), INVALID_PARAMS_CODE);
                assert!(
                    error.message().contains(limit),
                    "message `{}` does not name the limit `{limit}`",
                    error.message()
                );
            }
            other => panic!("expected an invalid params error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_owned_objects_refuses_too_large_filter() {
        let module = indexer_rpc_module();
        let query = IotaObjectResponseQuery::new(
            Some(IotaObjectDataFilter::ObjectIds(vec![
                ObjectId::ZERO;
                MAX_FILTER_NODES
            ])),
            None,
        );
        assert_invalid_params(
            module
                .call::<_, ObjectsPage>(
                    "iotax_getOwnedObjects",
                    (Address::ZERO, query, None::<ObjectId>, None::<usize>),
                )
                .await,
            &format!("{MAX_FILTER_NODES} nodes"),
        );
    }
}
