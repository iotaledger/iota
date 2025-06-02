// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc, time::Duration};

use anyhow::{anyhow, bail};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use iota_core::authority::AuthorityState;
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{
    IndexerApiOpenRpc, IndexerApiServer, JsonRpcMetrics, QUERY_MAX_RESULT_LIMIT, ReadApiServer,
    cap_page_limit, validate_limit,
};
use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaNameRecord, IotaObjectDataFilter,
    IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseQuery, ObjectsPage, Page,
    TransactionBlocksPage, TransactionFilter,
};
use iota_metrics::spawn_monitored_task;
use iota_names::{
    IotaNamesNft, IotaNamesRegistration, config::IotaNamesConfig, domain::Domain,
    error::IotaNamesError, registry::NameRecord,
};
use iota_open_rpc::Module;
use iota_storage::key_value_store::TransactionKeyValueStore;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::{DynamicFieldName, Field},
    error::IotaObjectResponseError,
    event::EventID,
};
use jsonrpsee::{
    PendingSubscriptionSink, RpcModule, SendTimeoutError, SubscriptionMessage,
    core::{RpcResult, SubscriptionResult},
};
use move_bytecode_utils::layout::TypeLayoutBuilder;
use move_core_types::language_storage::TypeTag;
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, instrument};

use crate::{
    IotaRpcModule,
    authority_state::{StateRead, StateReadResult},
    error::{Error, IotaRpcInputError},
    logger::FutureWithTracing as _,
};

async fn pipe_from_stream<T: Serialize>(
    pending: PendingSubscriptionSink,
    mut stream: impl Stream<Item = T> + Unpin,
) -> Result<(), anyhow::Error> {
    let sink = pending.accept().await?;

    loop {
        tokio::select! {
            _ = sink.closed() => break Ok(()),
            maybe_item = stream.next() => {
                let Some(item) = maybe_item else {
                    break Ok(());
                };

                let msg = SubscriptionMessage::from_json(&item)?;

                if let Err(e) = sink.send_timeout(msg, Duration::from_secs(60)).await {
                    match e {
                        // The subscription or connection was closed.
                        SendTimeoutError::Closed(_) => break Ok(()),
                        // The subscription send timeout expired
                        // the message is returned and you could save that message
                        // and retry again later.
                        SendTimeoutError::Timeout(_) => break Err(anyhow::anyhow!("Subscription timeout expired")),
                    }
                }
            }
        }
    }
}

pub fn spawn_subscription<S, T>(
    pending: PendingSubscriptionSink,
    rx: S,
    permit: Option<OwnedSemaphorePermit>,
) where
    S: Stream<Item = T> + Unpin + Send + 'static,
    T: Serialize + Send,
{
    spawn_monitored_task!(async move {
        let _permit = permit;
        match pipe_from_stream(pending, rx).await {
            Ok(_) => {
                debug!("Subscription completed.");
            }
            Err(err) => {
                debug!("Subscription failed: {err:?}");
            }
        }
    });
}
const DEFAULT_MAX_SUBSCRIPTIONS: usize = 100;

pub struct IndexerApi<R> {
    state: Arc<dyn StateRead>,
    read_api: R,
    transaction_kv_store: Arc<TransactionKeyValueStore>,
    iota_names_config: IotaNamesConfig,
    pub metrics: Arc<JsonRpcMetrics>,
    subscription_semaphore: Arc<Semaphore>,
}

impl<R: ReadApiServer> IndexerApi<R> {
    pub fn new(
        state: Arc<AuthorityState>,
        read_api: R,
        transaction_kv_store: Arc<TransactionKeyValueStore>,
        metrics: Arc<JsonRpcMetrics>,
        iota_names_config: IotaNamesConfig,
        max_subscriptions: Option<usize>,
    ) -> Self {
        let max_subscriptions = max_subscriptions.unwrap_or(DEFAULT_MAX_SUBSCRIPTIONS);
        Self {
            state,
            transaction_kv_store,
            read_api,
            metrics,
            iota_names_config,
            subscription_semaphore: Arc::new(Semaphore::new(max_subscriptions)),
        }
    }

    fn extract_values_from_dynamic_field_name(
        &self,
        name: DynamicFieldName,
    ) -> Result<(TypeTag, Vec<u8>), IotaRpcInputError> {
        let DynamicFieldName {
            type_: name_type,
            value,
        } = name;
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let layout = TypeLayoutBuilder::build_with_types(&name_type, epoch_store.module_cache())?;
        let iota_json_value = IotaJsonValue::new(value)?;
        let name_bcs_value = iota_json_value.to_bcs_bytes(&layout)?;
        Ok((name_type, name_bcs_value))
    }

    fn acquire_subscribe_permit(&self) -> anyhow::Result<OwnedSemaphorePermit> {
        match self.subscription_semaphore.clone().try_acquire_owned() {
            Ok(p) => Ok(p),
            Err(_) => bail!("Resources exhausted"),
        }
    }

    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectID,
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
    #[instrument(skip(self))]
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        query: Option<IotaObjectResponseQuery>,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<ObjectsPage> {
        async move {
            let limit =
                validate_limit(limit, *QUERY_MAX_RESULT_LIMIT).map_err(IotaRpcInputError::from)?;
            self.metrics.get_owned_objects_limit.report(limit as u64);
            let IotaObjectResponseQuery { filter, options } = query.unwrap_or_default();
            let options = options.unwrap_or_default();
            let mut objects =
                self.state
                    .get_owner_objects_with_limit(address, cursor, limit + 1, filter)?;

            // objects here are of size (limit + 1), where the last one is the cursor for
            // the next page
            let has_next_page = objects.len() > limit;
            objects.truncate(limit);
            let next_cursor = objects
                .last()
                .cloned()
                .map_or(cursor, |o_info| Some(o_info.object_id));

            let data = match options.is_not_in_object_info() {
                true => {
                    let object_ids = objects.iter().map(|obj| obj.object_id).collect();
                    self.read_api
                        .multi_get_objects(object_ids, Some(options))
                        .await
                        .map_err(|e| Error::Internal(anyhow!(e)))?
                }
                false => objects
                    .into_iter()
                    .map(|o_info| IotaObjectResponse::try_from((o_info, options.clone())))
                    .collect::<Result<Vec<IotaObjectResponse>, _>>()?,
            };

            self.metrics
                .get_owned_objects_result_size
                .report(data.len() as u64);
            self.metrics
                .get_owned_objects_result_size_total
                .inc_by(data.len() as u64);
            Ok(Page {
                data,
                next_cursor,
                has_next_page,
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
            self.metrics.query_tx_blocks_limit.report(limit as u64);
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
                .report(data.len() as u64);
            self.metrics
                .query_tx_blocks_result_size_total
                .inc_by(data.len() as u64);
            Ok(Page {
                data,
                next_cursor,
                has_next_page,
            })
        }
        .trace()
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
            self.metrics.query_events_limit.report(limit as u64);
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
                .report(data.len() as u64);
            self.metrics
                .query_events_result_size_total
                .inc_by(data.len() as u64);
            Ok(EventPage {
                data,
                next_cursor,
                has_next_page,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    fn subscribe_event(
        &self,
        sink: PendingSubscriptionSink,
        filter: EventFilter,
    ) -> SubscriptionResult {
        let permit = self.acquire_subscribe_permit()?;
        spawn_subscription(
            sink,
            self.state
                .get_subscription_handler()
                .subscribe_events(filter),
            Some(permit),
        );
        Ok(())
    }

    fn subscribe_transaction(
        &self,
        sink: PendingSubscriptionSink,
        filter: TransactionFilter,
    ) -> SubscriptionResult {
        // Validate unsupported filters
        if matches!(filter, TransactionFilter::Checkpoint(_)) {
            return Err("checkpoint filter is not supported".into());
        }

        let permit = self.acquire_subscribe_permit()?;
        spawn_subscription(
            sink,
            self.state
                .get_subscription_handler()
                .subscribe_transactions(filter),
            Some(permit),
        );
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_dynamic_fields(
        &self,
        parent_object_id: ObjectID,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<DynamicFieldPage> {
        async move {
            let limit = cap_page_limit(limit);
            self.metrics.get_dynamic_fields_limit.report(limit as u64);
            let mut data = self
                .state
                .get_dynamic_fields(parent_object_id, cursor, limit + 1)
                .map_err(Error::from)?;
            let has_next_page = data.len() > limit;
            data.truncate(limit);
            let next_cursor = data.last().cloned().map_or(cursor, |c| Some(c.0));
            self.metrics
                .get_dynamic_fields_result_size
                .report(data.len() as u64);
            self.metrics
                .get_dynamic_fields_result_size_total
                .inc_by(data.len() as u64);
            Ok(DynamicFieldPage {
                data: data.into_iter().map(|(_, w)| w.into()).collect(),
                next_cursor,
                has_next_page,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
    ) -> RpcResult<IotaObjectResponse> {
        self.get_dynamic_field_object(
            parent_object_id,
            name,
            Some(IotaObjectDataOptions::full_content()),
        )
        .await
    }

    #[instrument(skip(self))]
    async fn get_dynamic_field_object_v2(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        self.get_dynamic_field_object(parent_object_id, name, options)
            .await
    }

    async fn iota_names_lookup(&self, name: &str) -> RpcResult<Option<IotaNameRecord>> {
        let domain = name.parse::<Domain>().map_err(Error::from)?;

        // Construct the record id to lookup.
        let record_id = self.iota_names_config.record_field_id(&domain);

        let parent_record_id = domain
            .parent()
            .map(|parent_domain| self.iota_names_config.record_field_id(&parent_domain));

        // Keep record IDs alive by declaring both before creating futures
        let mut requests = vec![self.state.get_object(&record_id)];

        // We only want to fetch both the child and the parent if the domain is a
        // subdomain.
        if let Some(ref parent_record_id) = parent_record_id {
            requests.push(self.state.get_object(parent_record_id));
        }

        // Couldn't find a `multi_get_object` for this crate (looks like it uses a k,v
        // db) Always fetching both parent + child at the same time (even for
        // node subdomains), to avoid sequential db reads. We do this because we
        // do not know if the requested domain is a node subdomain or a leaf
        // subdomain, and we can save a trip to the db.
        let mut results = futures::future::try_join_all(requests)
            .await
            .map_err(Error::from)?;

        // Removing without checking vector len, since it is known (== 1 or 2 depending
        // on whether it is a subdomain or not).
        let Some(object) = results.remove(0) else {
            return Ok(None);
        };

        let name_record = NameRecord::try_from(object).map_err(Error::from)?;

        let current_timestamp_ms = self
            .get_latest_checkpoint_timestamp_ms()
            .map_err(Error::from)?;

        // Handling SLD names & node subdomains is the same (we handle them as `node`
        // records). We check their expiration, and if not expired, return the
        // target address.
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

    #[instrument(skip(self))]
    async fn iota_names_reverse_lookup(&self, address: IotaAddress) -> RpcResult<Option<String>> {
        let reverse_record_id = self.iota_names_config.reverse_record_field_id(&address);

        let Some(field_reverse_record_object) = self
            .state
            .get_object(&reverse_record_id)
            .await
            .map_err(Error::from)?
        else {
            return Ok(None);
        };

        let domain = field_reverse_record_object
            .to_rust::<Field<IotaAddress, Domain>>()
            .ok_or_else(|| Error::Unexpected(format!("malformed Object {reverse_record_id}")))?
            .value;

        let domain_name = domain.to_string();

        let resolved_record = self.iota_names_lookup(&domain_name).await?;

        // If looking up the domain returns an empty result, we return an empty result.
        if resolved_record.is_none() {
            return Ok(None);
        }

        Ok(Some(domain_name))
    }

    #[instrument(skip(self))]
    async fn iota_names_find_all_registration_nfts(
        &self,
        address: IotaAddress,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<ObjectsPage> {
        let query = IotaObjectResponseQuery {
            filter: Some(IotaObjectDataFilter::StructType(
                IotaNamesRegistration::type_(self.iota_names_config.package_address.into()),
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
