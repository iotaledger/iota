// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_json_rpc_api::GovernanceReadApiClient;
use iota_json_rpc_types::{DelegatedStake, IotaCommittee};
use iota_types::{
    base_types::IotaAddress, iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
};

use crate::{RpcClient, error::IotaRpcResult};

/// Governance API provides the staking functionality.
#[derive(Debug, Clone)]
pub struct GovernanceApi {
    api: Arc<RpcClient>,
}

impl GovernanceApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a list of [DelegatedStake] objects for the given address, or an
    /// error upon failure.
    pub async fn get_stakes(&self, owner: IotaAddress) -> IotaRpcResult<Vec<DelegatedStake>> {
        Ok(self.api.http.get_stakes(owner).await?)
    }

    /// Return the [IotaCommittee] information for the given `epoch`, or an
    /// error upon failure.
    ///
    /// The argument `epoch` is the known epoch id or `None` for the current
    /// epoch.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let committee_info = iota.governance_api().get_committee_info(None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_committee_info(
        &self,
        epoch: Option<BigInt<u64>>,
    ) -> IotaRpcResult<IotaCommittee> {
        Ok(self.api.http.get_committee_info(epoch).await?)
    }

    /// Return the latest IOTA system state object on-chain, or an error upon
    /// failure.
    ///
    /// Use this method to access system's information, such as the current
    /// epoch, the protocol version, the reference gas price, the total
    /// stake, active validators, and much more. See the
    /// [IotaSystemStateSummary] for all the available fields.
    pub async fn get_latest_iota_system_state(&self) -> IotaRpcResult<IotaSystemStateSummary> {
        Ok(self.api.http.get_latest_iota_system_state().await?)
    }

    /// Return the reference gas price for the network, or an error upon
    /// failure.
    pub async fn get_reference_gas_price(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_reference_gas_price().await?)
    }
}
