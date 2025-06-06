// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_json_rpc_api::GovernanceReadApiClient;
use iota_json_rpc_types::{DelegatedStake, DelegatedTimelockedStake, IotaCommittee, ValidatorApys};
use iota_types::{
    base_types::IotaAddress, iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
};

use crate::{RpcClient, error::IotaRpcResult};

/// Defines methods to get committee and staking info.
#[derive(Debug, Clone)]
pub struct GovernanceApi {
    api: Arc<RpcClient>,
}

impl GovernanceApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Get a list of delegated stakes for the given address.
    pub async fn get_stakes(&self, owner: IotaAddress) -> IotaRpcResult<Vec<DelegatedStake>> {
        Ok(self.api.http.get_stakes(owner).await?)
    }

    /// Get a list of delegated timelocked stakes for the given address.
    pub async fn get_timelocked_stakes(
        &self,
        owner: IotaAddress,
    ) -> IotaRpcResult<Vec<DelegatedTimelockedStake>> {
        Ok(self.api.http.get_timelocked_stakes(owner).await?)
    }

    /// Get committee information for the given epoch.
    ///
    /// The epoch defaults to the current epoch.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let committee_info = iota.governance_api().get_committee_info(None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_committee_info(
        &self,
        epoch: impl Into<Option<BigInt<u64>>>,
    ) -> IotaRpcResult<IotaCommittee> {
        Ok(self.api.http.get_committee_info(epoch.into()).await?)
    }

    /// Get the latest IOTA system state object on-chain.
    ///
    /// Use this method to access system information, such as the current epoch,
    /// the protocol version, the reference gas price, the total stake, active
    /// validators, and much more.
    #[allow(deprecated)]
    pub async fn get_latest_iota_system_state(&self) -> IotaRpcResult<IotaSystemStateSummary> {
        if self.api.info.iota_system_state_v2_support {
            Ok(self.api.http.get_latest_iota_system_state_v2().await?)
        } else {
            // Fallback to v1, v2 is not available on networks with protocol version < 5
            Ok(self
                .api
                .http
                .get_latest_iota_system_state()
                .await
                .map(IotaSystemStateSummary::from)?)
        }
    }

    /// Get the reference gas price for the network.
    pub async fn get_reference_gas_price(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_reference_gas_price().await?)
    }

    /// Get the validators APY.
    pub async fn get_validators_apy(&self) -> IotaRpcResult<ValidatorApys> {
        Ok(self.api.http.get_validators_apy().await?)
    }
}
