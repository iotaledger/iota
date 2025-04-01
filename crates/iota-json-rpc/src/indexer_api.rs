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
    DynamicFieldPage, EventFilter, EventPage, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockResponse, IotaTransactionBlockResponseQuery,
    ObjectsPage, Page, TransactionBlocksPage, TransactionFilter,
};
use iota_metrics::spawn_monitored_task;
use iota_open_rpc::Module;
use iota_storage::key_value_store::TransactionKeyValueStore;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::DynamicFieldName,
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
    authority_state::StateRead,
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
    pub metrics: Arc<JsonRpcMetrics>,
    subscription_semaphore: Arc<Semaphore>,
}

impl<R: ReadApiServer> IndexerApi<R> {
    pub fn new(
        state: Arc<AuthorityState>,
        read_api: R,
        transaction_kv_store: Arc<TransactionKeyValueStore>,
        metrics: Arc<JsonRpcMetrics>,
        max_subscriptions: Option<usize>,
    ) -> Self {
        let max_subscriptions = max_subscriptions.unwrap_or(DEFAULT_MAX_SUBSCRIPTIONS);
        Self {
            state,
            transaction_kv_store,
            read_api,
            metrics,
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
        async move {
            let (name_type, name_bcs_value) = self.extract_values_from_dynamic_field_name(name)?;

            let id = self
                .state
                .get_dynamic_field_object_id(parent_object_id, name_type, &name_bcs_value)
                .map_err(Error::from)?;
            // TODO(chris): add options to `get_dynamic_field_object` API as well
            if let Some(id) = id {
                self.read_api
                    .get_object(id, Some(IotaObjectDataOptions::full_content()))
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
}

impl<R: ReadApiServer> IotaRpcModule for IndexerApi<R> {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        IndexerApiOpenRpc::module_doc()
    }
}
