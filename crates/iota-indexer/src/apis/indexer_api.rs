// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_json_rpc::{
    name_service::{Domain, NameRecord, NameServiceConfig},
    IotaRpcModule,
};
use iota_json_rpc_api::{cap_page_limit, IndexerApiServer};
use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaObjectData, IotaObjectDataOptions,
    IotaObjectResponse, IotaObjectResponseQuery, IotaTransactionBlockResponseQuery, ObjectsPage,
    Page, TransactionBlocksPage, TransactionFilter,
};
use iota_open_rpc::Module;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::{DynamicFieldName, Field},
    error::IotaObjectResponseError,
    event::EventID,
    object::ObjectRead,
    TypeTag,
};
use jsonrpsee::{
    core::RpcResult,
    types::{error::INTERNAL_ERROR_CODE, ErrorObjectOwned},
    PendingSubscriptionSink, RpcModule,
};

use crate::{indexer_reader::IndexerReader, IndexerError};

pub(crate) struct IndexerApi {
    inner: IndexerReader,
    name_service_config: NameServiceConfig,
}

impl IndexerApi {
    pub fn new(inner: IndexerReader) -> Self {
        Self {
            inner,
            // TODO allow configuring for other networks
            name_service_config: Default::default(),
        }
    }

    async fn get_owned_objects_internal(
        &self,
        address: IotaAddress,
        query: Option<IotaObjectResponseQuery>,
        cursor: Option<ObjectID>,
        limit: usize,
    ) -> RpcResult<ObjectsPage> {
        let IotaObjectResponseQuery { filter, options } = query.unwrap_or_default();
        let options = options.unwrap_or_default();
        let objects = self
            .inner
            .get_owned_objects_in_blocking_task(address, filter, cursor, limit + 1)
            .await?;
        let mut objects = self
            .inner
            .spawn_blocking(move |this| {
                objects
                    .into_iter()
                    .map(|object| object.try_into_object_read(&this))
                    .collect::<Result<Vec<_>, _>>()
            })
            .await?;
        let has_next_page = objects.len() > limit;
        objects.truncate(limit);

        let next_cursor = objects.last().map(|o_read| o_read.object_id());
        let mut parallel_tasks = Vec::with_capacity(objects.len());
        async fn check_read_obj(
            obj: ObjectRead,
            reader: IndexerReader,
            options: IotaObjectDataOptions,
        ) -> anyhow::Result<IotaObjectResponse> {
            match obj {
                ObjectRead::NotExists(id) => Ok(IotaObjectResponse::new_with_error(
                    IotaObjectResponseError::NotExists { object_id: id },
                )),
                ObjectRead::Exists(object_ref, o, layout) => {
                    if options.show_display {
                        match reader.get_display_fields(&o, &layout).await {
                            Ok(rendered_fields) => {
                                Ok(IotaObjectResponse::new_with_data(IotaObjectData::new(
                                    object_ref,
                                    o,
                                    layout,
                                    options,
                                    rendered_fields,
                                )?))
                            }
                            Err(e) => Ok(IotaObjectResponse::new(
                                Some(IotaObjectData::new(object_ref, o, layout, options, None)?),
                                Some(IotaObjectResponseError::DisplayError {
                                    error: e.to_string(),
                                }),
                            )),
                        }
                    } else {
                        Ok(IotaObjectResponse::new_with_data(IotaObjectData::new(
                            object_ref, o, layout, options, None,
                        )?))
                    }
                }
                ObjectRead::Deleted((object_id, version, digest)) => Ok(
                    IotaObjectResponse::new_with_error(IotaObjectResponseError::Deleted {
                        object_id,
                        version,
                        digest,
                    }),
                ),
            }
        }
        for obj in objects {
            parallel_tasks.push(tokio::task::spawn(check_read_obj(
                obj,
                self.inner.clone(),
                options.clone(),
            )));
        }
        let data = futures::future::join_all(parallel_tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None))?;

        Ok(Page {
            data,
            next_cursor,
            has_next_page,
        })
    }
}

#[async_trait]
impl IndexerApiServer for IndexerApi {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        query: Option<IotaObjectResponseQuery>,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<ObjectsPage> {
        let limit = cap_page_limit(limit);
        if limit == 0 {
            return Ok(ObjectsPage::empty());
        }
        self.get_owned_objects_internal(address, query, cursor, limit)
            .await
    }

    async fn query_transaction_blocks(
        &self,
        query: IotaTransactionBlockResponseQuery,
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<TransactionBlocksPage> {
        let limit = cap_page_limit(limit);
        if limit == 0 {
            return Ok(TransactionBlocksPage::empty());
        }
        let mut results = self
            .inner
            .query_transaction_blocks_in_blocking_task(
                query.filter,
                query.options.unwrap_or_default(),
                cursor,
                limit + 1,
                descending_order.unwrap_or(false),
            )
            .await?;

        let has_next_page = results.len() > limit;
        results.truncate(limit);
        let next_cursor = results.last().map(|o| o.digest);
        Ok(Page {
            data: results,
            next_cursor,
            has_next_page,
        })
    }

    async fn query_events(
        &self,
        query: EventFilter,
        // exclusive cursor if `Some`, otherwise start from the beginning
        cursor: Option<EventID>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<EventPage> {
        let limit = cap_page_limit(limit);
        if limit == 0 {
            return Ok(EventPage::empty());
        }
        let descending_order = descending_order.unwrap_or(false);
        let mut results = self
            .inner
            .query_events_in_blocking_task(query, cursor, limit + 1, descending_order)
            .await?;

        let has_next_page = results.len() > limit;
        results.truncate(limit);
        let next_cursor = results.last().map(|o| o.id);
        Ok(Page {
            data: results,
            next_cursor,
            has_next_page,
        })
    }

    async fn get_dynamic_fields(
        &self,
        parent_object_id: ObjectID,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<DynamicFieldPage> {
        let limit = cap_page_limit(limit);
        if limit == 0 {
            return Ok(DynamicFieldPage::empty());
        }
        let mut results = self
            .inner
            .get_dynamic_fields_in_blocking_task(parent_object_id, cursor, limit + 1)
            .await?;

        let has_next_page = results.len() > limit;
        results.truncate(limit);
        let next_cursor = results.last().map(|o| o.object_id);
        Ok(Page {
            data: results,
            next_cursor,
            has_next_page,
        })
    }

    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
    ) -> RpcResult<IotaObjectResponse> {
        let name_bcs_value = self
            .inner
            .bcs_name_from_dynamic_field_name_in_blocking_task(&name)
            .await?;

        // Try as Dynamic Field
        let id = iota_types::dynamic_field::derive_dynamic_field_id(
            parent_object_id,
            &name.type_,
            &name_bcs_value,
        )
        .expect("deriving dynamic field id can't fail");

        let options = iota_json_rpc_types::IotaObjectDataOptions::full_content();
        match self.inner.get_object_read_in_blocking_task(id).await? {
            iota_types::object::ObjectRead::NotExists(_)
            | iota_types::object::ObjectRead::Deleted(_) => {}
            iota_types::object::ObjectRead::Exists(object_ref, o, layout) => {
                return Ok(IotaObjectResponse::new_with_data(
                    IotaObjectData::new(object_ref, o, layout, options, None).map_err(|e| {
                        ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                    })?,
                ));
            }
        }

        // Try as Dynamic Field Object
        let dynamic_object_field_struct =
            iota_types::dynamic_field::DynamicFieldInfo::dynamic_object_field_wrapper(name.type_);
        let dynamic_object_field_type = TypeTag::Struct(Box::new(dynamic_object_field_struct));
        let dynamic_object_field_id = iota_types::dynamic_field::derive_dynamic_field_id(
            parent_object_id,
            &dynamic_object_field_type,
            &name_bcs_value,
        )
        .expect("deriving dynamic field id can't fail");
        match self
            .inner
            .get_object_read_in_blocking_task(dynamic_object_field_id)
            .await?
        {
            iota_types::object::ObjectRead::NotExists(_)
            | iota_types::object::ObjectRead::Deleted(_) => {}
            iota_types::object::ObjectRead::Exists(object_ref, o, layout) => {
                return Ok(IotaObjectResponse::new_with_data(
                    IotaObjectData::new(object_ref, o, layout, options, None).map_err(|e| {
                        ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                    })?,
                ));
            }
        }

        Ok(IotaObjectResponse::new_with_error(
            iota_types::error::IotaObjectResponseError::DynamicFieldNotFound { parent_object_id },
        ))
    }

    async fn subscribe_event(&self, _sink: PendingSubscriptionSink, _filter: EventFilter) {}

    async fn subscribe_transaction(
        &self,
        _sink: PendingSubscriptionSink,
        _filter: TransactionFilter,
    ) {
    }

    async fn resolve_name_service_address(&self, name: String) -> RpcResult<Option<IotaAddress>> {
        // TODO(manos): Implement new logic.
        let domain = name
            .parse::<Domain>()
            .map_err(IndexerError::NameServiceError)?;

        let record_id = self.name_service_config.record_field_id(&domain);

        let field_record_object = match self.inner.get_object_in_blocking_task(record_id).await? {
            Some(o) => o,
            None => return Ok(None),
        };

        let record = NameRecord::try_from(field_record_object)
            .map_err(|e| IndexerError::PersistentStorageDataCorruptionError(e.to_string()))?;

        Ok(record.target_address)
    }

    async fn resolve_name_service_names(
        &self,
        address: IotaAddress,
        _cursor: Option<ObjectID>,
        _limit: Option<usize>,
    ) -> RpcResult<Page<String, ObjectID>> {
        let reverse_record_id = self
            .name_service_config
            .reverse_record_field_id(address.as_ref());

        let field_reverse_record_object = match self
            .inner
            .get_object_in_blocking_task(reverse_record_id)
            .await?
        {
            Some(o) => o,
            None => {
                return Ok(Page {
                    data: vec![],
                    next_cursor: None,
                    has_next_page: false,
                });
            }
        };

        let domain = field_reverse_record_object
            .to_rust::<Field<IotaAddress, Domain>>()
            .ok_or_else(|| {
                IndexerError::PersistentStorageDataCorruptionError(format!(
                    "Malformed Object {reverse_record_id}"
                ))
            })?
            .value;

        Ok(Page {
            data: vec![domain.to_string()],
            next_cursor: None,
            has_next_page: false,
        })
    }
}

impl IotaRpcModule for IndexerApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::IndexerApiOpenRpc::module_doc()
    }
}
