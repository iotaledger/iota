// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_core::authority::AuthorityState;
use iota_json_rpc_api::{BridgeReadApiOpenRpc, BridgeReadApiServer, JsonRpcMetrics};
use iota_open_rpc::Module;
use iota_types::bridge::{BridgeSummary, BridgeTrait, get_bridge_obj_initial_shared_version};
use jsonrpsee::{RpcModule, core::RpcResult};
use tracing::instrument;

use crate::{IotaRpcModule, authority_state::StateRead, error::Error, logger::FutureWithTracing};

#[derive(Clone)]
pub struct BridgeReadApi {
    state: Arc<dyn StateRead>,
    pub metrics: Arc<JsonRpcMetrics>,
}

impl BridgeReadApi {
    pub fn new(state: Arc<AuthorityState>, metrics: Arc<JsonRpcMetrics>) -> Self {
        Self { state, metrics }
    }
}

#[async_trait]
impl BridgeReadApiServer for BridgeReadApi {
    #[instrument(skip(self))]
    async fn get_latest_bridge(&self) -> RpcResult<BridgeSummary> {
        async move {
            self.state
                .get_bridge()
                .map_err(Error::from)?
                .try_into_bridge_summary()
                .map_err(Error::from)
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_bridge_object_initial_shared_version(&self) -> RpcResult<u64> {
        async move {
            Ok(
                get_bridge_obj_initial_shared_version(self.state.get_object_store())?
                    .ok_or(Error::UnexpectedError(
                        "Failed to find Bridge object initial version".to_string(),
                    ))?
                    .into(),
            )
        }
        .trace()
        .await
    }
}

impl IotaRpcModule for BridgeReadApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        BridgeReadApiOpenRpc::module_doc()
    }
}
