// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_json_rpc::{IotaRpcModule, error::IotaRpcInputError};
use iota_json_rpc_api::{
    ExtendedApiServer, QUERY_MAX_RESULT_LIMIT_CHECKPOINTS, internal_error, validate_limit,
};
use iota_json_rpc_types::{
    AddressMetrics, EpochInfo, EpochMetrics, EpochMetricsPage, EpochPage, MoveCallMetrics,
    NetworkMetrics, Page, ParticipationMetrics,
};
use iota_open_rpc::Module;
use iota_sdk_types::Address;
use iota_types::{claim_registry::key_id_from_prefixed_bytes, iota_serde::BigInt};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::read::IndexerReader;

pub(crate) struct ExtendedApi {
    inner: IndexerReader,
}

impl ExtendedApi {
    pub fn new(inner: IndexerReader) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl ExtendedApiServer for ExtendedApi {
    async fn get_epochs(
        &self,
        cursor: Option<BigInt<u64>>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<EpochPage> {
        let limit =
            validate_limit(limit, QUERY_MAX_RESULT_LIMIT_CHECKPOINTS).map_err(internal_error)?;
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

    async fn get_epoch_metrics(
        &self,
        cursor: Option<BigInt<u64>>,
        limit: Option<usize>,
        descending_order: Option<bool>,
    ) -> RpcResult<EpochMetricsPage> {
        let limit =
            validate_limit(limit, QUERY_MAX_RESULT_LIMIT_CHECKPOINTS).map_err(internal_error)?;
        let epochs = self
            .inner
            .spawn_blocking(move |this| {
                this.get_epochs(
                    cursor.map(|x| *x),
                    limit + 1,
                    descending_order.unwrap_or(false),
                )
            })
            .await?;

        let mut epoch_metrics = epochs
            .into_iter()
            .map(|e| EpochMetrics {
                epoch: e.epoch,
                epoch_total_transactions: e.epoch_total_transactions,
                first_checkpoint_id: e.first_checkpoint_id,
                epoch_start_timestamp: e.epoch_start_timestamp,
                end_of_epoch_info: e.end_of_epoch_info,
            })
            .collect::<Vec<_>>();

        let has_next_page = epoch_metrics.len() > limit;
        epoch_metrics.truncate(limit);
        let next_cursor = epoch_metrics.last().map(|e| e.epoch);
        Ok(Page {
            data: epoch_metrics,
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

    async fn get_network_metrics(&self) -> RpcResult<NetworkMetrics> {
        let network_metrics = self
            .inner
            .spawn_blocking(|this| this.get_latest_network_metrics())
            .await?;
        Ok(network_metrics)
    }

    async fn get_move_call_metrics(&self) -> RpcResult<MoveCallMetrics> {
        let move_call_metrics = self
            .inner
            .spawn_blocking(|this| this.get_latest_move_call_metrics())
            .await?;
        Ok(move_call_metrics)
    }

    async fn get_latest_address_metrics(&self) -> RpcResult<AddressMetrics> {
        let latest_address_metrics = self
            .inner
            .spawn_blocking(|this| this.get_latest_address_metrics())
            .await?;
        Ok(latest_address_metrics)
    }

    async fn get_checkpoint_address_metrics(&self, checkpoint: u64) -> RpcResult<AddressMetrics> {
        let checkpoint_address_metrics = self
            .inner
            .spawn_blocking(move |this| this.get_checkpoint_address_metrics(checkpoint))
            .await?;
        Ok(checkpoint_address_metrics)
    }

    async fn get_all_epoch_address_metrics(
        &self,
        descending_order: Option<bool>,
    ) -> RpcResult<Vec<AddressMetrics>> {
        let all_epoch_address_metrics = self
            .inner
            .spawn_blocking(move |this| this.get_all_epoch_address_metrics(descending_order))
            .await?;
        Ok(all_epoch_address_metrics)
    }

    async fn get_total_transactions(&self) -> RpcResult<BigInt<u64>> {
        let latest_checkpoint = self
            .inner
            .spawn_blocking(|this| this.get_latest_checkpoint())
            .await?;
        Ok(latest_checkpoint.network_total_transactions.into())
    }

    async fn get_participation_metrics(&self) -> RpcResult<ParticipationMetrics> {
        self.inner
            .spawn_blocking(|this| this.get_participation_metrics())
            .await
            .map_err(Into::into)
    }

    async fn get_accounts_by_public_key(
        &self,
        public_key: Base64,
        include_unlinked: Option<bool>,
    ) -> RpcResult<Vec<Address>> {
        let public_key = public_key
            .to_vec()
            .map_err(|e| IotaRpcInputError::GenericInvalid(format!("Invalid base64: {e}")))?;
        let key_id = key_id_from_prefixed_bytes(&public_key).ok_or_else(|| {
            IotaRpcInputError::GenericInvalid("Public key must not be empty".to_string())
        })?;

        let links = self
            .inner
            .get_account_key_links_in_blocking_task(
                key_id.as_bytes().to_vec(),
                include_unlinked.unwrap_or(false),
            )
            .await?;

        links
            .into_iter()
            .map(|link| {
                <[u8; Address::LENGTH]>::try_from(link.account_id.as_slice())
                    .map(Address::new)
                    .map_err(|_| {
                        internal_error(format!(
                            "account_key_links holds a malformed account id of {} bytes",
                            link.account_id.len()
                        ))
                    })
            })
            .collect()
    }
}

impl IotaRpcModule for ExtendedApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::ExtendedApiOpenRpc::module_doc()
    }
}
