// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use iota_grpc_client::Client as GrpcClient;
use iota_json_rpc::{IotaRpcModule, error::IotaRpcInputError};
use iota_json_rpc_api::{QUERY_MAX_RESULT_LIMIT, ReadApiServer, internal_error};
use iota_json_rpc_types::{
    Checkpoint, CheckpointId, CheckpointPage, IotaEvent, IotaGetPastObjectRequest, IotaObjectData,
    IotaObjectDataOptions, IotaObjectResponse, IotaPastObjectResponse,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ProtocolConfigResponse,
};
use iota_open_rpc::Module;
use iota_protocol_config::{ProtocolConfig, ProtocolVersion};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::{ChainIdentifier, TransactionDigest},
    error::IotaObjectResponseError,
    iota_serde::BigInt,
    object::{ObjectRead, PastObjectRead},
};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::{
    errors::{IndexerError, IndexerResult},
    models::objects::StoredObject,
    read::IndexerReader,
};

#[derive(Clone)]
pub struct ReadApi {
    inner: IndexerReader,
    fullnode_grpc_client: GrpcClient,
}

impl ReadApi {
    /// Creates a new instance of ReadApi with a fullnode RPC client which can
    /// be either JSON-RPC or gRPC.
    pub fn new(inner: IndexerReader, fullnode_grpc_client: GrpcClient) -> Self {
        Self {
            inner,
            fullnode_grpc_client,
        }
    }

    async fn get_checkpoint(&self, id: CheckpointId) -> Result<Checkpoint, IndexerError> {
        match self.inner.get_checkpoint_with_fallback(id).await {
            Ok(Some(checkpoint)) => Ok(checkpoint),
            Ok(None) => Err(IndexerError::InvalidArgument(format!(
                "Checkpoint {id:?} not found"
            ))),
            Err(e) => Err(e),
        }
    }

    async fn get_latest_checkpoint(&self) -> Result<Checkpoint, IndexerError> {
        self.inner
            .spawn_blocking(|this| this.get_latest_checkpoint())
            .await
    }

    async fn get_chain_identifier(&self) -> RpcResult<ChainIdentifier> {
        Ok(self.inner.get_chain_identifier_in_blocking_task().await?)
    }

    async fn object_read_to_object_response(
        &self,
        object_read: ObjectRead,
        options: IotaObjectDataOptions,
    ) -> RpcResult<IotaObjectResponse> {
        match object_read {
            ObjectRead::NotExists(id) => Ok(IotaObjectResponse::new_with_error(
                IotaObjectResponseError::NotExists { object_id: id },
            )),
            ObjectRead::Exists(object_ref, o, layout) => {
                let mut display_fields = None;
                if options.show_display {
                    match self.inner.get_display_fields(&o, &layout).await {
                        Ok(rendered_fields) => display_fields = Some(rendered_fields),
                        Err(e) => {
                            return Ok(IotaObjectResponse::new(
                                Some(
                                    IotaObjectData::new(object_ref, o, layout, &options, None)
                                        .map_err(internal_error)?,
                                ),
                                Some(IotaObjectResponseError::Display {
                                    error: e.to_string(),
                                }),
                            ));
                        }
                    }
                }
                Ok(IotaObjectResponse::new_with_data(
                    IotaObjectData::new(object_ref, o, layout, &options, display_fields)
                        .map_err(internal_error)?,
                ))
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

    async fn past_object_read_to_response(
        &self,
        options: Option<IotaObjectDataOptions>,
        past_object_read: PastObjectRead,
    ) -> RpcResult<IotaPastObjectResponse> {
        let options = options.unwrap_or_default();

        match past_object_read {
            PastObjectRead::ObjectNotExists(id) => Ok(IotaPastObjectResponse::ObjectNotExists(id)),

            PastObjectRead::ObjectDeleted(object_ref) => {
                Ok(IotaPastObjectResponse::ObjectDeleted(object_ref.into()))
            }

            PastObjectRead::VersionFound(object_ref, object, layout) => {
                let display_fields = if options.show_display {
                    let rendered_fields = self
                        .inner
                        .get_display_fields(&object, &layout)
                        .await
                        .map_err(internal_error)?;

                    Some(rendered_fields)
                } else {
                    None
                };

                Ok(IotaPastObjectResponse::VersionFound(
                    IotaObjectData::new(object_ref, object, layout, &options, display_fields)
                        .map_err(internal_error)?,
                ))
            }

            PastObjectRead::VersionNotFound(object_id, version) => {
                Ok(IotaPastObjectResponse::VersionNotFound(object_id, version))
            }

            PastObjectRead::VersionTooHigh {
                object_id,
                asked_version,
                latest_version,
            } => Ok(IotaPastObjectResponse::VersionTooHigh {
                object_id,
                asked_version,
                latest_version,
            }),
        }
    }

    /// Checks if the transaction is indexed on node through fullnode gRPC API.
    async fn is_transaction_indexed_on_node(
        &self,
        digest: TransactionDigest,
    ) -> IndexerResult<bool> {
        match self
            .fullnode_grpc_client
            .get_transactions(&[digest.into()], Some("transaction.digest"))
            .await
        {
            Ok(txns) => {
                let executed_tx = txns.into_inner().pop().ok_or_else(|| {
                    IndexerError::Grpc("there should be one tx lookup response".into())
                })?;

                Ok(executed_tx.transaction()?.digest()? == digest.into())
            }
            Err(e) => {
                if matches!(e, iota_grpc_client::Error::Server(ref e) if  tonic::Code::from_i32(e.code) == tonic::Code::NotFound)
                {
                    return Ok(false);
                }
                Err(IndexerError::from(e))
            }
        }
    }
}

#[async_trait]
impl ReadApiServer for ReadApi {
    async fn get_object(
        &self,
        object_id: ObjectID,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse> {
        let object_read = self
            .inner
            .get_object_read_in_blocking_task(object_id)
            .await?;
        self.object_read_to_object_response(object_read, options.unwrap_or_default())
            .await
    }

    async fn multi_get_objects(
        &self,
        object_ids: Vec<ObjectID>,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<Vec<IotaObjectResponse>> {
        if object_ids.len() > *QUERY_MAX_RESULT_LIMIT {
            return Err(
                IotaRpcInputError::SizeLimitExceeded(QUERY_MAX_RESULT_LIMIT.to_string()).into(),
            );
        }

        // Doesn't take care of missing objects.
        let stored_objects = self
            .inner
            .multi_get_objects_in_blocking_task(object_ids.clone())
            .await?;

        // Map the returned `StoredObject`s to `ObjectID`
        let object_map: Arc<HashMap<ObjectID, StoredObject>> = Arc::new(
            stored_objects
                .into_iter()
                .map(|obj| {
                    let object_id = ObjectID::try_from(obj.object_id.clone()).map_err(|_| {
                        IndexerError::PersistentStorageDataCorruption(format!(
                            "failed to parse ObjectID: {:?}",
                            obj.object_id
                        ))
                    })?;
                    Ok::<(ObjectID, StoredObject), IndexerError>((object_id, obj))
                })
                .collect::<Result<_, IndexerError>>()?,
        );

        let options = options.unwrap_or_default();
        let resolver = self.inner.package_resolver();

        // Create a future for each requested object id
        let futures = object_ids.into_iter().map(|object_id| {
            let options = options.clone();
            let maybe_stored = object_map.get(&object_id).cloned();
            async move {
                match maybe_stored {
                    Some(stored) => {
                        let object_read = stored.try_into_object_read(resolver).await?;
                        self.object_read_to_object_response(object_read, options)
                            .await
                    }
                    None => {
                        self.object_read_to_object_response(
                            ObjectRead::NotExists(object_id),
                            options,
                        )
                        .await
                    }
                }
            }
        });

        futures::future::try_join_all(futures).await
    }

    async fn get_total_transaction_blocks(&self) -> RpcResult<BigInt<u64>> {
        let checkpoint = self.get_latest_checkpoint().await?;
        Ok(BigInt::from(checkpoint.network_total_transactions))
    }

    async fn is_transaction_indexed_on_node(&self, digest: TransactionDigest) -> RpcResult<bool> {
        self.is_transaction_indexed_on_node(digest)
            .await
            .map_err(Into::into)
    }

    async fn get_transaction_block(
        &self,
        digest: TransactionDigest,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let options = options.unwrap_or_default();
        let txn = self
            .inner
            .get_single_transaction_block_response_with_fallback(digest, options)
            .await?;

        let txn = txn.ok_or_else(|| {
            IndexerError::InvalidArgument(format!("Transaction {digest} not found"))
        })?;

        Ok(txn)
    }

    async fn multi_get_transaction_blocks(
        &self,
        digests: Vec<TransactionDigest>,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> RpcResult<Vec<IotaTransactionBlockResponse>> {
        let num_digests = digests.len();
        if num_digests > *QUERY_MAX_RESULT_LIMIT {
            Err(IotaRpcInputError::SizeLimitExceeded(
                QUERY_MAX_RESULT_LIMIT.to_string(),
            ))?
        }

        let options = options.unwrap_or_default();
        let txns = self
            .inner
            .multi_get_transaction_block_response_in_blocking_task(digests, options)
            .await?;

        Ok(txns)
    }

    async fn try_get_past_object(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaPastObjectResponse> {
        let past_object_read = self
            .inner
            .get_past_object_read_with_fallback(object_id, version, false)
            .await?;

        self.past_object_read_to_response(options, past_object_read)
            .await
    }

    async fn try_get_object_before_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> RpcResult<IotaPastObjectResponse> {
        let past_object_read = self
            .inner
            .get_past_object_read_with_fallback(object_id, version, true)
            .await?;

        self.past_object_read_to_response(None, past_object_read)
            .await
    }

    async fn try_multi_get_past_objects(
        &self,
        past_objects: Vec<IotaGetPastObjectRequest>,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<Vec<IotaPastObjectResponse>> {
        let mut responses = Vec::with_capacity(past_objects.len());

        for request in past_objects {
            let past_object_read = self
                .inner
                .get_past_object_read_with_fallback(request.object_id, request.version, false)
                .await?;

            responses.push(
                self.past_object_read_to_response(options.clone(), past_object_read)
                    .await?,
            );
        }

        Ok(responses)
    }

    async fn get_latest_checkpoint_sequence_number(&self) -> RpcResult<BigInt<u64>> {
        let checkpoint = self.get_latest_checkpoint().await?;
        Ok(BigInt::from(checkpoint.sequence_number))
    }

    async fn get_checkpoint(&self, id: CheckpointId) -> RpcResult<Checkpoint> {
        Ok(self.get_checkpoint(id).await?)
    }

    async fn get_checkpoints(
        &self,
        cursor: Option<BigInt<u64>>,
        limit: Option<usize>,
        descending_order: bool,
    ) -> RpcResult<CheckpointPage> {
        let cursor = cursor.map(BigInt::into_inner);
        let limit = iota_json_rpc_api::validate_limit(
            limit,
            iota_json_rpc_api::QUERY_MAX_RESULT_LIMIT_CHECKPOINTS,
        )
        .map_err(IotaRpcInputError::from)?;

        let mut checkpoints = self
            .inner
            .get_checkpoints_with_fallback(cursor, limit + 1, descending_order)
            .await?;

        let has_next_page = checkpoints.len() > limit;
        checkpoints.truncate(limit);

        let next_cursor = checkpoints.last().map(|d| d.sequence_number.into());

        Ok(CheckpointPage {
            data: checkpoints,
            next_cursor,
            has_next_page,
        })
    }

    async fn get_events(&self, transaction_digest: TransactionDigest) -> RpcResult<Vec<IotaEvent>> {
        self.inner
            .get_transaction_events_with_fallback(transaction_digest)
            .await
            .map_err(Into::into)
    }

    async fn get_protocol_config(
        &self,
        version: Option<BigInt<u64>>,
    ) -> RpcResult<ProtocolConfigResponse> {
        let chain = self.get_chain_identifier().await?.chain();
        let version = if let Some(version) = version {
            (*version).into()
        } else {
            let latest_epoch = self
                .inner
                .spawn_blocking(|this| this.get_latest_epoch_info_from_db())
                .await?;
            (latest_epoch.protocol_version as u64).into()
        };

        ProtocolConfig::get_for_version_if_supported(version, chain)
            .ok_or(IotaRpcInputError::ProtocolVersionUnsupported(
                ProtocolVersion::MIN.as_u64(),
                ProtocolVersion::MAX.as_u64(),
            ))
            .map_err(Into::into)
            .map(ProtocolConfigResponse::from)
    }

    async fn get_chain_identifier(&self) -> RpcResult<String> {
        self.get_chain_identifier().await.map(|id| id.to_string())
    }
}

impl IotaRpcModule for ReadApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::ReadApiOpenRpc::module_doc()
    }
}
