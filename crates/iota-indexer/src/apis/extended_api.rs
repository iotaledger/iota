// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::r2d2::R2D2Connection;
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::{validate_limit, ExtendedApiServer, QUERY_MAX_RESULT_LIMIT_CHECKPOINTS};
use iota_json_rpc_types::{
    CheckpointedObjectID, EpochInfo, EpochPage, IotaObjectResponseQuery, Page, QueryObjectsPage,
};
use iota_open_rpc::Module;
use iota_types::iota_serde::BigInt;
use jsonrpsee::{core::RpcResult, RpcModule};

use crate::indexer_reader::IndexerReader;

pub(crate) struct ExtendedApi<T: R2D2Connection + 'static> {
    inner: IndexerReader<T>,
}

impl<T: R2D2Connection> ExtendedApi<T> {
    pub fn new(inner: IndexerReader<T>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<T: R2D2Connection + 'static> ExtendedApiServer for ExtendedApi<T> {
    async fn get_epochs(
        &self,
        cursor: Option<BigInt<u64>>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<EpochPage> {
        let limit = validate_limit(limit, QUERY_MAX_RESULT_LIMIT_CHECKPOINTS)?;
        let mut epochs = self
            .inner
            .spawn_blocking(move |this| {
                this.get_epochs(
                    cursor.map(|x| *x),
                    limit + 1,
                    descending_order.unwrap_or(false),
                )
            })
            .await?;

        let has_next_page = epochs.len() > limit;
        epochs.truncate(limit);
        let next_cursor = epochs.last().map(|e| e.epoch);
        Ok(Page {
            data: epochs,
            next_cursor: next_cursor.map(|id| id.into()),
            has_next_page,
        })
    }

    async fn get_current_epoch(&self) -> RpcResult<EpochInfo> {
        let stored_epoch = self
            .inner
            .spawn_blocking(|this| this.get_latest_epoch_info_from_db())
            .await?;
        EpochInfo::try_from(stored_epoch).map_err(Into::into)
    }

    async fn query_objects(
        &self,
        _query: IotaObjectResponseQuery,
        _cursor: Option<CheckpointedObjectID>,
        _limit: Option<usize>,
    ) -> RpcResult<QueryObjectsPage> {
        Err(jsonrpsee::types::error::CallError::Custom(
            jsonrpsee::types::error::ErrorCode::MethodNotFound.into(),
        )
        .into())
    }

    async fn get_total_transactions(&self) -> RpcResult<BigInt<u64>> {
        let latest_checkpoint = self
            .inner
            .spawn_blocking(|this| this.get_latest_checkpoint())
            .await?;
        Ok(latest_checkpoint.network_total_transactions.into())
    }
}

impl<T: R2D2Connection> IotaRpcModule for ExtendedApi<T> {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::ExtendedApiOpenRpc::module_doc()
    }
}
