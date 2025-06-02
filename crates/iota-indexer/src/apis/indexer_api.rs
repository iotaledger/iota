// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use async_trait::async_trait;
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::{IndexerApiServer, cap_page_limit, error_object_from_rpc, internal_error};
use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaNameRecord, IotaObjectData, IotaObjectDataFilter,
    IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery,
    IotaTransactionBlockResponseQuery, ObjectsPage, Page, TransactionBlocksPage, TransactionFilter,
};
use iota_names::{
    IotaNamesNft, IotaNamesRegistration, config::IotaNamesConfig, domain::Domain,
    error::IotaNamesError, registry::NameRecord,
};
use iota_open_rpc::Module;
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::{DynamicFieldName, Field},
    error::IotaObjectResponseError,
    event::EventID,
    object::ObjectRead,
};
use jsonrpsee::{
    PendingSubscriptionSink, RpcModule,
    core::{RpcResult, SubscriptionResult, client::Error as RpcClientError},
};
use tap::TapFallible;

use crate::{errors::IndexerError, indexer_reader::IndexerReader};

pub(crate) struct IndexerApi {
    inner: IndexerReader,
    iota_names_config: IotaNamesConfig,
}

impl IndexerApi {
    pub fn new(inner: IndexerReader, iota_names_config: IotaNamesConfig) -> Self {
        Self {
            inner,
            iota_names_config,
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

        let mut object_futures = vec![];
        for object in objects {
            object_futures.push(tokio::task::spawn(
                object.try_into_object_read(self.inner.package_resolver()),
            ));
        }
        let mut objects = futures::future::try_join_all(object_futures)
            .await
            .map_err(|e| {
                tracing::error!("Error joining object read futures.");
                RpcClientError::Custom(format!("Error joining object read futures. {e}"))
            })
            .map_err(error_object_from_rpc)?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| tracing::error!("Error converting object to object read: {e}"))?;
        let has_next_page = objects.len() > limit;
        objects.truncate(limit);

        let next_cursor = objects.last().map(|o_read| o_read.object_id());
        let construct_response_tasks = objects.into_iter().map(|object| {
            tokio::task::spawn(construct_object_response(
                object,
                self.inner.clone(),
                options.clone(),
            ))
        });
        let data = futures::future::try_join_all(construct_response_tasks)
            .await
            .map_err(internal_error)?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;

        Ok(Page {
            data,
            next_cursor,
            has_next_page,
        })
    }

    async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        let name_bcs_value = self.inner.bcs_name_from_dynamic_field_name(&name).await?;

        // Try as Dynamic Field
        let id = iota_types::dynamic_field::derive_dynamic_field_id(
            parent_object_id,
            &name.type_,
            &name_bcs_value,
        )
        .map_err(internal_error)?;

        let options = options.unwrap_or_default();

        match self.inner.get_object_read_in_blocking_task(id).await? {
            ObjectRead::NotExists(_) | ObjectRead::Deleted(_) => {}
            ObjectRead::Exists(object_ref, o, layout) => {
                return Ok(IotaObjectResponse::new_with_data(
                    IotaObjectData::new(object_ref, o, layout, options, None)
                        .map_err(internal_error)?,
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
        .map_err(internal_error)?;

        match self
            .inner
            .get_object_read_in_blocking_task(dynamic_object_field_id)
            .await?
        {
            ObjectRead::NotExists(_) | ObjectRead::Deleted(_) => {}
            ObjectRead::Exists(object_ref, o, layout) => {
                return Ok(IotaObjectResponse::new_with_data(
                    IotaObjectData::new(object_ref, o, layout, options, None)
                        .map_err(internal_error)?,
                ));
            }
        }

        Ok(IotaObjectResponse::new_with_error(
            IotaObjectResponseError::DynamicFieldNotFound { parent_object_id },
        ))
    }
}

async fn construct_object_response(
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
                    Ok(rendered_fields) => Ok(IotaObjectResponse::new_with_data(
                        IotaObjectData::new(object_ref, o, layout, options, rendered_fields)?,
                    )),
                    Err(e) => Ok(IotaObjectResponse::new(
                        Some(IotaObjectData::new(object_ref, o, layout, options, None)?),
                        Some(IotaObjectResponseError::Display {
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
            data: results.into_iter().map(Into::into).collect(),
            next_cursor,
            has_next_page,
        })
    }

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

    async fn get_dynamic_field_object_v2(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        self.get_dynamic_field_object(parent_object_id, name, options)
            .await
    }

    fn subscribe_event(
        &self,
        _sink: PendingSubscriptionSink,
        _filter: EventFilter,
    ) -> SubscriptionResult {
        Err("empty subscription".into())
    }

    fn subscribe_transaction(
        &self,
        _sink: PendingSubscriptionSink,
        _filter: TransactionFilter,
    ) -> SubscriptionResult {
        Err("empty subscription".into())
    }

    async fn iota_names_lookup(&self, name: &str) -> RpcResult<Option<IotaNameRecord>> {
        let domain: Domain = name.parse().map_err(IndexerError::IotaNames)?;

        // Construct the record id to lookup.
        let record_id = self.iota_names_config.record_field_id(&domain);

        // Gather the requests to fetch in the multi_get_objs.
        let mut requests = vec![record_id];

        // We only want to fetch both the child and the parent if the domain is a
        // subdomain.
        let parent_record_id = domain.parent().map(|parent_domain| {
            let parent_record_id = self.iota_names_config.record_field_id(&parent_domain);
            requests.push(parent_record_id);
            parent_record_id
        });

        // Fetch both parent (if subdomain) and child records in a single get query.
        // We do this as we do not know if the subdomain is a node or leaf record.
        let mut domain_object_map = self
            .inner
            .multi_get_objects_in_blocking_task(requests)
            .await?
            .into_iter()
            .map(iota_types::object::Object::try_from)
            .try_fold(HashMap::new(), |mut map, res| {
                let obj = res?;
                map.insert(obj.id(), obj.try_into()?);
                Ok::<HashMap<ObjectID, NameRecord>, IndexerError>(map)
            })?;

        // Extract the name record for the provided domain
        let Some(name_record) = domain_object_map.remove(&record_id) else {
            return Ok(None);
        };

        // get latest timestamp to check expiration.
        let current_timestamp = self
            .inner
            .get_latest_checkpoint_timestamp_ms_in_blocking_task()
            .await?;

        // If the provided domain is a `node` record, we can check for expiration
        if !name_record.is_leaf_record() {
            return if !name_record.is_node_expired(current_timestamp) {
                Ok(Some(name_record.into()))
            } else {
                Err(IndexerError::IotaNames(IotaNamesError::NameExpired).into())
            };
        } else {
            // Handle the `leaf` record case which requires to check the parent for
            // expiration.
            let parent_record_id = parent_record_id.expect("leaf record should have a parent");
            // If the parent record is not found for the existing leaf, we consider it
            // expired.
            let parent_record = domain_object_map
                .remove(&parent_record_id)
                .ok_or_else(|| IndexerError::IotaNames(IotaNamesError::NameExpired))?;

            if parent_record.is_valid_leaf_parent(&name_record)
                && !parent_record.is_node_expired(current_timestamp)
            {
                return Ok(Some(name_record.into()));
            } else {
                return Err(IndexerError::IotaNames(IotaNamesError::NameExpired).into());
            }
        }
    }

    async fn iota_names_reverse_lookup(&self, address: IotaAddress) -> RpcResult<Option<String>> {
        let reverse_record_id = self.iota_names_config.reverse_record_field_id(&address);

        let Some(field_reverse_record_object) = self
            .inner
            .get_object_in_blocking_task(reverse_record_id)
            .await?
        else {
            return Ok(None);
        };

        let domain = field_reverse_record_object
            .to_rust::<Field<IotaAddress, Domain>>()
            .ok_or_else(|| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Malformed Object {reverse_record_id}"
                ))
            })?
            .value;

        let domain_name = domain.to_string();

        // Tries to resolve the name, to verify it is not expired.
        let resolved_record = self.iota_names_lookup(&domain_name).await?;

        // If we do not have a resolved address, we do not include the domain in the
        // result.
        if resolved_record.is_none() {
            return Ok(None);
        }

        Ok(Some(domain_name))
    }

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

impl IotaRpcModule for IndexerApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::IndexerApiOpenRpc::module_doc()
    }
}
